use axum::Router;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue};
use axum::response::Response;
use axum::routing::any;

use crate::AppState;
use crate::auth::KeyInfo;
use crate::proxy::UpstreamAuth;
use crate::routes::common::{ProxyConfig, handle_proxy};
use crate::token;
use error::ProxyError;

/// Router for /anthropic/* -> https://api.anthropic.com/*
pub fn anthropic_router() -> Router<AppState> {
    Router::new().route("/{*wildcard}", any(anthropic_proxy_handler))
}

const ALLOWED_ENDPOINTS: &[&str] = &["v1/messages"];

async fn anthropic_proxy_handler(
    State(state): State<AppState>,
    info: KeyInfo,
    Path(path): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    let access_token = token::get_or_refresh_token(&state).await?;

    let mut extra_headers = HeaderMap::new();
    extra_headers.insert(
        "anthropic-version",
        HeaderValue::from_static("2023-06-01"),
    );
    extra_headers.insert(
        "anthropic-beta",
        HeaderValue::from_static("claude-code-20250219,oauth-2025-04-20,interleaved-thinking-2025-05-14,redact-thinking-2026-02-12,context-management-2025-06-27,prompt-caching-scope-2026-01-05,advanced-tool-use-2025-11-20,effort-2025-11-24"),
    );

    handle_proxy(
        &state,
        &info,
        &path,
        headers,
        body,
        ProxyConfig {
            allowed_endpoints: ALLOWED_ENDPOINTS,
            upstream_base: "https://api.anthropic.com",
            auth: UpstreamAuth::Bearer(access_token),
            log_label: "anthropic",
            extra_headers: Some(extra_headers),
        },
    )
    .await
}
