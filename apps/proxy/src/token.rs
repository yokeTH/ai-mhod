use redis::AsyncCommands;

use crate::AppState;
use error::ProxyError;

const REDIS_KEY: &str = "anthropic:token";
const LOCK_KEY: &str = "anthropic:token:lock";
const LOCK_TTL_SECS: u64 = 30;
const EXPIRY_BUFFER_SECS: i64 = 60;
const SCOPE: &str =
    "user:file_upload user:inference user:mcp_servers user:profile user:sessions:claude_code";
const TOKEN_URL: &str = "https://platform.claude.com/v1/oauth/token";
const LOCK_POLL_INTERVAL_MS: u64 = 200;
const LOCK_POLL_MAX_ATTEMPTS: u32 = 50;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CachedToken {
    access_token: String,
    refresh_token: String,
    expires_at: i64,
}

#[derive(serde::Serialize)]
struct TokenRequest<'a> {
    grant_type: &'a str,
    refresh_token: &'a str,
    scope: &'a str,
    client_id: &'a str,
}

#[derive(serde::Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
}

pub async fn get_or_refresh_token(state: &AppState) -> Result<String, ProxyError> {
    let mut conn = state
        .redis
        .clone()
        .ok_or_else(|| ProxyError::TokenError("redis not configured".to_string()))?;

    if let Some(access_token) = read_cached_access_token(&mut conn).await? {
        return Ok(access_token);
    }

    let lock_value = uuid::Uuid::new_v4().to_string();
    let locked = acquire_lock(&mut conn, &lock_value).await?;

    if locked {
        if let Some(access_token) = read_cached_access_token(&mut conn).await? {
            release_lock(&mut conn, &lock_value).await?;
            return Ok(access_token);
        }

        let result = do_refresh(&mut conn, state).await;
        release_lock(&mut conn, &lock_value).await?;
        return result;
    }

    wait_for_unlock(&mut conn).await?;
    read_cached_access_token(&mut conn).await?.ok_or_else(|| {
        ProxyError::TokenError("token still unavailable after refresh wait".to_string())
    })
}

async fn read_cached_access_token(
    conn: &mut redis::aio::ConnectionManager,
) -> Result<Option<String>, ProxyError> {
    let cached: Option<String> = conn
        .get(REDIS_KEY)
        .await
        .map_err(|e| ProxyError::TokenError(format!("redis get failed: {e}")))?;

    let Some(ref json) = cached else {
        return Ok(None);
    };

    let token = serde_json::from_str::<CachedToken>(json)
        .map_err(|e| ProxyError::TokenError(format!("failed to parse cached token: {e}")))?;

    let now = chrono::Utc::now().timestamp();
    if token.expires_at > now + EXPIRY_BUFFER_SECS {
        Ok(Some(token.access_token))
    } else {
        Ok(None)
    }
}

async fn acquire_lock(
    conn: &mut redis::aio::ConnectionManager,
    lock_value: &str,
) -> Result<bool, ProxyError> {
    let acquired: bool = redis::cmd("SET")
        .arg(LOCK_KEY)
        .arg(lock_value)
        .arg("NX")
        .arg("EX")
        .arg(LOCK_TTL_SECS)
        .query_async(conn)
        .await
        .map_err(|e| ProxyError::TokenError(format!("redis lock failed: {e}")))?;
    Ok(acquired)
}

async fn release_lock(
    conn: &mut redis::aio::ConnectionManager,
    lock_value: &str,
) -> Result<(), ProxyError> {
    let script = redis::Script::new(
        "if redis.call('get', KEYS[1]) == ARGV[1] then return redis.call('del', KEYS[1]) else return 0 end",
    );
    script
        .key(LOCK_KEY)
        .arg(lock_value)
        .invoke_async::<()>(&mut *conn)
        .await
        .map_err(|e| ProxyError::TokenError(format!("redis unlock failed: {e}")))?;
    Ok(())
}

async fn wait_for_unlock(conn: &mut redis::aio::ConnectionManager) -> Result<(), ProxyError> {
    for _ in 0..LOCK_POLL_MAX_ATTEMPTS {
        let exists: bool = conn
            .exists(LOCK_KEY)
            .await
            .map_err(|e| ProxyError::TokenError(format!("redis exists failed: {e}")))?;
        if !exists {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(LOCK_POLL_INTERVAL_MS)).await;
    }
    Err(ProxyError::TokenError(
        "timed out waiting for token refresh lock".to_string(),
    ))
}

async fn do_refresh(
    conn: &mut redis::aio::ConnectionManager,
    state: &AppState,
) -> Result<String, ProxyError> {
    let refresh_token = get_refresh_token(conn, &state.config.anthropic_refresh_token).await?;

    let req = TokenRequest {
        grant_type: "refresh_token",
        refresh_token: &refresh_token,
        scope: SCOPE,
        client_id: &state.config.anthropic_client_id,
    };

    let response = state
        .client
        .post(TOKEN_URL)
        .header("Content-Type", "application/json")
        .header("User-Agent", "axios/1.13.6")
        .header("Accept", "application/json, text/plain, */*")
        .json(&req)
        .send()
        .await
        .map_err(|e| ProxyError::TokenError(format!("token request failed: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(ProxyError::TokenError(format!(
            "token endpoint returned {status}: {body}"
        )));
    }

    let token_resp: TokenResponse = response
        .json()
        .await
        .map_err(|e| ProxyError::TokenError(format!("failed to parse token response: {e}")))?;

    let now = chrono::Utc::now().timestamp();
    let new_token = CachedToken {
        access_token: token_resp.access_token,
        refresh_token: token_resp.refresh_token,
        expires_at: now + token_resp.expires_in as i64 - EXPIRY_BUFFER_SECS,
    };

    let json = serde_json::to_string(&new_token)
        .map_err(|e| ProxyError::TokenError(format!("failed to serialize token: {e}")))?;

    let (): () = conn
        .set(REDIS_KEY, &json)
        .await
        .map_err(|e| ProxyError::TokenError(format!("redis set token failed: {e}")))?;

    Ok(new_token.access_token)
}

async fn get_refresh_token(
    conn: &mut redis::aio::ConnectionManager,
    env_refresh_token: &str,
) -> Result<String, ProxyError> {
    let cached: Option<String> = conn
        .get(REDIS_KEY)
        .await
        .map_err(|e| ProxyError::TokenError(format!("redis get failed: {e}")))?;

    let refresh_token = cached
        .as_deref()
        .and_then(|j| serde_json::from_str::<CachedToken>(j).ok())
        .map(|t| t.refresh_token);

    Ok(refresh_token.unwrap_or_else(|| env_refresh_token.to_string()))
}
