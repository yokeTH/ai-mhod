use axum::Router;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::any;

use crate::AppState;
use crate::auth::KeyInfo;
use crate::proxy::UpstreamAuth;
use crate::routes::common::{ProxyConfig, handle_proxy};
use error::ProxyError;

/// Router for /zai/* -> https://api.z.ai/*
pub fn zai_router() -> Router<AppState> {
    Router::new().route("/{*wildcard}", any(zai_proxy_handler))
}

const ALLOWED_ENDPOINTS: &[&str] = &["api/anthropic"];

async fn zai_proxy_handler(
    State(state): State<AppState>,
    info: KeyInfo,
    Path(path): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    handle_proxy(
        &state,
        &info,
        &path,
        headers,
        body,
        ProxyConfig {
            allowed_endpoints: ALLOWED_ENDPOINTS,
            upstream_base: "https://api.z.ai",
            auth: UpstreamAuth::ApiKey(state.config.upstream_api_key.clone()),
            log_label: "zai",
            extra_headers: None,
        },
    )
    .await
}

/// Health check endpoint.
pub async fn health() -> &'static str {
    "ok"
}
