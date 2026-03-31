use axum::extract::{FromRequestParts, Request, State};
use axum::http::HeaderMap;
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::Response;

use crate::error::ProxyError;
use crate::AppState;

/// Middleware that validates the API key from either `x-api-key` or `Authorization: Bearer` headers.
/// Looks up the key in the database and stores the user_id in request extensions.
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

    let user_id = state
        .repo
        .lookup_key(&key)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "DB error looking up API key");
            ProxyError::Unauthorized
        })?
        .ok_or(ProxyError::Unauthorized)?;

    request.extensions_mut().insert(UserId(user_id));
    Ok(next.run(request).await)
}

/// Extracted user ID stored in request extensions.
#[derive(Clone, Copy, Debug)]
pub struct UserId(pub i64);

impl FromRequestParts<AppState> for UserId {
    type Rejection = ProxyError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<UserId>()
            .copied()
            .ok_or(ProxyError::Unauthorized)
    }
}
