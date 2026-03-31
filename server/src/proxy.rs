use axum::body::Body;
use axum::http::HeaderMap;
use axum::response::Response;
use futures_util::StreamExt;

use crate::error::ProxyError;
use crate::metrics::RequestMetrics;
use crate::AppState;
use model::anthropic::AnthropicResponse;

/// Forward a request to an upstream URL. Passthrough body, swap auth header.
pub async fn proxy_to(
    state: &AppState,
    url: &str,
    headers: &HeaderMap,
    body: axum::body::Bytes,
    is_stream: bool,
    metrics: &RequestMetrics,
) -> Result<Response, ProxyError> {
    let upstream_api_key = &state.config.upstream_api_key;

    // Forward all client headers, only swap auth
    let mut req_builder = state.client.post(url);
    for (key, val) in headers.iter() {
        let key = key.as_str();
        // Skip hop-by-hop and auth headers — we set our own
        if matches!(key, "host" | "connection" | "transfer-encoding" | "content-length"
            | "x-api-key" | "authorization")
        {
            continue;
        }
        req_builder = req_builder.header(key, val);
    }
    req_builder = req_builder
        .header("x-api-key", upstream_api_key)
        .body(body);

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
            // Accumulate raw SSE text — parsing happens in finish() after stream ends
            if let Ok(text) = std::str::from_utf8(&bytes) {
                metrics_stream.append_sse(text);
            }
            Ok(bytes)
        });

        // Chain a terminator that parses the accumulated SSE and logs metrics.
        // This runs after all upstream chunks have been consumed.
        let stream = mapped.chain(futures_util::stream::once(async move {
            metrics_end.finish();
            Ok::<_, std::io::Error>(axum::body::Bytes::new())
        }));

        let body = Body::from_stream(stream);
        Ok(Response::builder()
            .status(200)
            .header("Content-Type", "text/event-stream")
            .header("Cache-Control", "no-cache")
            .header("Connection", "keep-alive")
            .body(body)
            .unwrap())
    } else {
        let bytes = response
            .bytes()
            .await
            .map_err(|e| ProxyError::UpstreamError(e.to_string()))?;

        // Parse for metrics only — still return raw bytes
        if let Ok(resp) = serde_json::from_slice::<AnthropicResponse>(&bytes) {
            if let Some(u) = &resp.usage {
                metrics.set_tokens(u.input_tokens, u.output_tokens, u.cache_read_input_tokens);
            }
        }

        Ok(Response::builder()
            .status(200)
            .header("Content-Type", "application/json")
            .body(Body::from(bytes))
            .unwrap())
    }
}
