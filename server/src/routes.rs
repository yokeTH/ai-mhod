use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::any;
use axum::Router;

use crate::auth::UserId;
use crate::error::ProxyError;
use crate::metrics::RequestMetrics;
use crate::proxy::proxy_to;
use crate::AppState;

/// Router for /zai/* -> https://api.z.ai/*
pub fn zai_router() -> Router<AppState> {
    Router::new().route("/{*wildcard}", any(zai_proxy_handler))
}

/// Proxy handler: /zai/{path} -> https://api.z.ai/{path}
async fn zai_proxy_handler(
    State(state): State<AppState>,
    UserId(user_id): UserId,
    Path(path): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    let url = format!("https://api.z.ai/{path}");

    let is_stream = serde_json::from_slice::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v.get("stream").and_then(|s| s.as_bool()))
        .unwrap_or(false);

    let model = serde_json::from_slice::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v.get("model").and_then(|m| m.as_str()).map(String::from))
        .unwrap_or_default();

    let metrics = RequestMetrics::new(&model, is_stream, user_id, state.usage_tx.clone());

    tracing::info!(
        request_id = %metrics.request_id(),
        user_id = user_id,
        model = %model,
        stream = is_stream,
        upstream = %url,
        "Incoming request"
    );

    let result = proxy_to(&state, &url, &headers, body, is_stream, &metrics).await;

    match &result {
        Ok(_) => {
            // For non-streaming responses, finish metrics here.
            // For streaming, the stream terminator in proxy.rs calls finish()
            // after all SSE chunks are consumed.
            if !is_stream {
                metrics.finish();
            }
        }
        Err(e) => tracing::error!(request_id = %metrics.request_id(), error = %e, "Request failed"),
    }

    result
}

/// Health check endpoint.
pub async fn health() -> &'static str {
    "ok"
}
