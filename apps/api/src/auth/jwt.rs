use std::sync::Arc;

use axum::extract::{FromRequestParts, Request, State};
use axum::http::HeaderMap;
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::Response;
use jsonwebtoken::{DecodingKey, Validation, decode, jwk::JwkSet};
use serde::Deserialize;

use crate::AppState;
use error::ProxyError;

#[derive(Debug, Deserialize)]
struct Claims {
    sub: String,
}

pub struct JwksCache {
    jwks_url: String,
    state: Arc<tokio::sync::Mutex<CachedJwks>>,
}

struct CachedJwks {
    jwks: Option<JwkSet>,
    fetched_at: std::time::Instant,
}

impl JwksCache {
    pub fn new(jwks_url: String) -> Self {
        Self {
            jwks_url,
            state: Arc::new(tokio::sync::Mutex::new(CachedJwks {
                jwks: None,
                fetched_at: std::time::Instant::now() - std::time::Duration::from_secs(3600),
            })),
        }
    }

    async fn get_jwks(&self) -> Option<JwkSet> {
        let cache = self.state.lock().await;
        if cache.fetched_at.elapsed().as_secs() < 3600 {
            cache.jwks.clone()
        } else {
            None
        }
    }

    async fn fetch_jwks(&self) -> anyhow::Result<JwkSet> {
        if let Some(jwks) = self.get_jwks().await {
            return Ok(jwks);
        }

        let resp = reqwest::get(&self.jwks_url).await?;
        let jwks: JwkSet = resp.json().await?;

        let mut cache = self.state.lock().await;
        cache.jwks = Some(jwks.clone());
        cache.fetched_at = std::time::Instant::now();

        Ok(jwks)
    }

    pub async fn verify_token(&self, token: &str) -> anyhow::Result<String> {
        let jwks = self.fetch_jwks().await?;

        let header = jsonwebtoken::decode_header(token)?;
        let kid = header
            .kid
            .ok_or_else(|| anyhow::anyhow!("missing kid in token header"))?;

        let jwk = jwks
            .find(&kid)
            .ok_or_else(|| anyhow::anyhow!("JWK not found for kid: {kid}"))?;

        let decoding_key = DecodingKey::from_jwk(jwk)?;
        let mut validation = Validation::new(header.alg);
        validation.validate_exp = true;
        validation.validate_aud = false;

        let token_data = decode::<Claims>(token, &decoding_key, &validation)?;
        Ok(token_data.claims.sub)
    }
}

/// Middleware that validates a JWT Bearer token via Keycloak JWKS.
pub async fn require_jwt(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Result<Response, ProxyError> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer ").map(|s| s.trim().to_string()))
        .ok_or(ProxyError::Unauthorized)?;

    let sub: String = state.jwks_cache.verify_token(&token).await.map_err(|e| {
        tracing::warn!(error = %e, "JWT verification failed");
        ProxyError::Unauthorized
    })?;

    let user_id = state
        .repo
        .lookup_user_by_keycloak_sub(&sub)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "DB error looking up keycloak sub");
            ProxyError::Unauthorized
        })?
        .ok_or(ProxyError::Unauthorized)?;

    request.extensions_mut().insert(JwtUser { user_id });
    Ok(next.run(request).await)
}

/// Extracted JWT user info stored in request extensions.
#[derive(Clone, Debug)]
pub struct JwtUser {
    pub user_id: String,
}

impl FromRequestParts<AppState> for JwtUser {
    type Rejection = ProxyError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<JwtUser>()
            .cloned()
            .ok_or(ProxyError::Unauthorized)
    }
}
