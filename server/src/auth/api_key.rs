use axum::extract::{FromRequestParts, Request, State};
use axum::http::HeaderMap;
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::Response;

use crate::error::ProxyError;
use crate::AppState;

/// Middleware that validates the API key from either `x-api-key` or `Authorization: Bearer` headers.
/// Looks up the key in the database and stores KeyInfo (user_id + api_key) in request extensions.
pub async fn require_api_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Result<Response, ProxyError> {
    let key = headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| {
            headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer ").map(|s| s.trim().to_string()))
        })
        .ok_or(ProxyError::Unauthorized)?;

    let (user_id, api_key_id, revoked) = state
        .repo
        .lookup_key(&key)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "DB error looking up API key");
            ProxyError::Unauthorized
        })?
        .ok_or(ProxyError::Unauthorized)?;

    if revoked {
        return Err(ProxyError::Unauthorized);
    }

    request.extensions_mut().insert(KeyInfo { user_id, api_key_id });
    Ok(next.run(request).await)
}

/// Extracted key info stored in request extensions.
#[derive(Clone, Debug)]
pub struct KeyInfo {
    pub user_id: String,
    pub api_key_id: String,
}

impl FromRequestParts<AppState> for KeyInfo {
    type Rejection = ProxyError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<KeyInfo>()
            .cloned()
            .ok_or(ProxyError::Unauthorized)
    }
}
