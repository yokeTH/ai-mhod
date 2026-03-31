use axum::extract::{Request, State};
use axum::http::HeaderMap;
use axum::middleware::Next;
use axum::response::Response;

use crate::error::ProxyError;
use crate::AppState;

/// Middleware that validates the API key from either `x-api-key` or `Authorization: Bearer` headers.
pub async fn require_api_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request,
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

    if !state.config.allowed_api_keys.contains(&key) {
        tracing::warn!(api_key = %key, "Unauthorized request");
        return Err(ProxyError::Unauthorized);
    }

    Ok(next.run(request).await)
}
