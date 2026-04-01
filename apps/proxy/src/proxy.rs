use axum::body::Body;
use axum::http::{HeaderMap, HeaderName, HeaderValue};
use axum::response::Response;
use futures_util::StreamExt;

use crate::AppState;
use crate::dto::claude::AnthropicResponse;
use crate::metrics::RequestMetrics;
use error::ProxyError;

/// Upstream authentication method to inject into proxied requests.
#[derive(Clone)]
pub enum UpstreamAuth {
    ApiKey(String),
    Bearer(String),
}

/// Check if a request header should be skipped (case-insensitive).
fn should_skip_header(name: &str) -> bool {
    matches!(
        name.to_lowercase().as_str(),
        "host"
            | "connection"
            | "transfer-encoding"
            | "content-length"
            | "x-api-key"
            | "authorization"
    )
}

/// Check if a response header should be skipped (hop-by-hop / overridden).
fn should_skip_response_header(name: &str) -> bool {
    matches!(
        name.to_lowercase().as_str(),
        "transfer-encoding" | "connection" | "content-length"
    )
}

/// Forward a request to an upstream URL. Passthrough body, swap auth header.
pub async fn proxy_to(
    state: &AppState,
    url: &str,
    headers: &HeaderMap,
    body: axum::body::Bytes,
    is_stream: bool,
    metrics: &RequestMetrics,
    auth: UpstreamAuth,
) -> Result<Response, ProxyError> {
    let mut req_builder = state.client.post(url);

    // Set default Anthropic headers if not provided by client
    if !headers
        .iter()
        .any(|(k, _)| k.as_str().eq_ignore_ascii_case("anthropic-version"))
    {
        req_builder = req_builder.header("anthropic-version", "2023-06-01");
    }
    if !headers.iter().any(|(k, _)| {
        k.as_str()
            .eq_ignore_ascii_case("anthropic-dangerous-direct-browser-access")
    }) {
        req_builder = req_builder.header("anthropic-dangerous-direct-browser-access", "true");
    }

    // Forward client headers
    for (key, val) in headers.iter() {
        if should_skip_header(key.as_str()) {
            continue;
        }
        req_builder = req_builder.header(key, val);
    }

    let req_builder = match auth {
        UpstreamAuth::ApiKey(key) => req_builder.header("x-api-key", key),
        UpstreamAuth::Bearer(token) => {
            req_builder.header("authorization", format!("Bearer {token}"))
        }
    };
    let req_builder = req_builder.body(body);

    let response = req_builder
        .send()
        .await
        .map_err(|e| ProxyError::UpstreamError(e.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        tracing::error!(status = %status, body = %body, "Upstream error");
        return Err(ProxyError::UpstreamError(format!(
            "upstream returned {status}: {body}"
        )));
    }

    // Capture upstream headers before response is consumed
    let upstream_headers: Vec<(HeaderName, HeaderValue)> = response
        .headers()
        .iter()
        .filter(|(key, _)| !should_skip_response_header(key.as_str()))
        .map(|(key, val)| (key.clone(), val.clone()))
        .collect();
    let has_content_type = response.headers().contains_key("content-type");

    if is_stream {
        let metrics_stream = metrics.clone();
        let metrics_end = metrics.clone();

        let mapped = response.bytes_stream().map(move |chunk_result| {
            let bytes = match chunk_result {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!(error = %e, "Stream chunk error");
                    return Ok::<_, std::io::Error>(axum::body::Bytes::new());
                }
            };
            if let Ok(text) = std::str::from_utf8(&bytes) {
                metrics_stream.append_sse(text);
            }
            Ok(bytes)
        });

        let stream = mapped.chain(futures_util::stream::once(async move {
            metrics_end.finish();
            Ok::<_, std::io::Error>(axum::body::Bytes::new())
        }));

        let body = Body::from_stream(stream);

        let mut builder = Response::builder().status(status);
        for (key, val) in upstream_headers {
            builder = builder.header(key, val);
        }
        builder = builder
            .header("Content-Type", "text/event-stream")
            .header("Cache-Control", "no-cache")
            .header("Connection", "keep-alive");

        Ok(builder.body(body).expect("stream response builder is valid"))
    } else {
        let bytes = response
            .bytes()
            .await
            .map_err(|e| ProxyError::UpstreamError(e.to_string()))?;

        if let Ok(resp) = serde_json::from_slice::<AnthropicResponse>(&bytes)
            && let Some(u) = &resp.usage
        {
            metrics.set_tokens(u.input_tokens, u.output_tokens, u.cache_read_input_tokens);
        }

        let mut builder = Response::builder().status(status);
        for (key, val) in upstream_headers {
            builder = builder.header(key, val);
        }
        if !has_content_type {
            builder = builder.header("Content-Type", "application/json");
        }

        Ok(builder.body(Body::from(bytes)).expect("json response builder is valid"))
    }
}
