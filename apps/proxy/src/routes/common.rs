use axum::body::Bytes;
use axum::http::HeaderMap;
use axum::response::Response;

use crate::AppState;
use crate::auth::KeyInfo;
use crate::metrics::RequestMetrics;
use crate::proxy::UpstreamAuth;
use crate::proxy::proxy_to;
use error::ProxyError;

pub(crate) struct ProxyConfig<'a> {
    pub allowed_endpoints: &'a [&'a str],
    pub upstream_base: &'a str,
    pub auth: UpstreamAuth,
    pub log_label: &'a str,
    pub extra_headers: Option<HeaderMap>,
}

pub(crate) async fn handle_proxy(
    state: &AppState,
    info: &KeyInfo,
    path: &str,
    headers: HeaderMap,
    body: Bytes,
    config: ProxyConfig<'_>,
) -> Result<Response, ProxyError> {
    if !config
        .allowed_endpoints
        .iter()
        .any(|ep| path.starts_with(ep))
    {
        return Err(ProxyError::UpstreamError(format!(
            "path not allowed: {path}"
        )));
    }

    let url = format!("{}/{}", config.upstream_base, path);

    let headers = match config.extra_headers {
        Some(extra) => {
            let mut merged = headers;
            for (key, val) in extra.iter() {
                merged.insert(key.clone(), val.clone());
            }
            merged
        }
        None => headers,
    };

    let body_json = serde_json::from_slice::<serde_json::Value>(&body).ok();
    let is_stream = body_json
        .as_ref()
        .and_then(|v| v.get("stream").and_then(|s| s.as_bool()))
        .unwrap_or(false);

    let model = body_json
        .and_then(|v| v.get("model").and_then(|m| m.as_str()).map(String::from))
        .unwrap_or_default();

    let metrics = RequestMetrics::new(
        &model,
        is_stream,
        &info.user_id,
        &info.api_key_id,
        state.usage_tx.clone(),
    );

    tracing::info!(
        request_id = %metrics.request_id(),
        user_id = %info.user_id,
        api_key_id = %info.api_key_id,
        model = %model,
        stream = is_stream,
        upstream = %url,
        "Incoming {} request", config.log_label,
    );

    let result = proxy_to(
        state,
        &url,
        &headers,
        body,
        is_stream,
        &metrics,
        config.auth,
    )
    .await;

    match &result {
        Ok(_) => {
            if !is_stream {
                metrics.finish();
            }
        }
        Err(e) => tracing::error!(
            request_id = %metrics.request_id(),
            error = %e,
            "{} request failed", config.log_label,
        ),
    }

    result
}
