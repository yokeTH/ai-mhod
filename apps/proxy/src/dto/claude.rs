use serde::Deserialize;

/// Minimal response struct — only extract what we need for metrics.
#[derive(Debug, Deserialize)]
pub struct AnthropicResponse {
    pub model: Option<String>,
    pub usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
pub struct AnthropicUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,
}

/// SSE streaming event — we only care about `message_start` and `message_delta`
/// for extracting token usage.
#[derive(Debug, Deserialize)]
pub struct StreamEvent {
    #[serde(rename = "type")]
    pub event_type: Option<String>,
    pub message: Option<StreamMessage>,
    pub delta: Option<StreamDelta>,
    pub usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
pub struct StreamMessage {
    pub usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
pub struct StreamDelta {
    pub stop_reason: Option<String>,
}

/// Parsed SSE line: `data: {...}`
fn parse_sse_data(line: &str) -> Option<&str> {
    line.strip_prefix("data: ").filter(|v| !v.is_empty())
}

/// SSE error event from upstream.
#[derive(Debug, Deserialize)]
pub struct StreamErrorData {
    pub error: Option<StreamErrorDetail>,
    pub request_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct StreamErrorDetail {
    pub code: Option<String>,
    pub message: Option<String>,
}

/// Extracted usage from a chunk of SSE text.
#[derive(Default)]
pub struct StreamUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,
    /// If the stream contained an `event: error`, this holds the error message.
    pub error: Option<String>,
}

/// Extract token usage from SSE text. Looks at `message_start` and `message_delta` events.
/// Also detects `event: error` lines from the upstream.
pub fn extract_stream_usage(sse_text: &str) -> StreamUsage {
    let mut result = StreamUsage::default();
    let mut current_event: Option<String> = None;

    for line in sse_text.lines() {
        // Track the SSE event type (e.g. "event: error", "event: message_start")
        if let Some(event_type) = line.strip_prefix("event: ") {
            current_event = Some(event_type.to_string());
            continue;
        }

        let Some(json_str) = parse_sse_data(line) else {
            continue;
        };

        // Handle error events: `event: error\ndata: {"error":{...}}`
        if current_event.as_deref() == Some("error") {
            if let Ok(err) = serde_json::from_str::<StreamErrorData>(json_str) {
                let msg = err
                    .error
                    .as_ref()
                    .and_then(|e| e.message.as_deref())
                    .unwrap_or("unknown stream error");
                let code = err
                    .error
                    .as_ref()
                    .and_then(|e| e.code.as_deref())
                    .unwrap_or("unknown");
                result.error = Some(format!("[{code}] {msg}"));
            } else {
                result.error = Some(format!("unparseable stream error: {json_str}"));
            }
            current_event = None;
            continue;
        }

        current_event = None;

        let Ok(evt) = serde_json::from_str::<StreamEvent>(json_str) else {
            continue;
        };

        match evt.event_type.as_deref() {
            Some("message_start") => {
                if let Some(msg) = &evt.message
                    && let Some(u) = &msg.usage
                {
                    result.input_tokens = u.input_tokens.or(result.input_tokens);
                    result.output_tokens = u.output_tokens.or(result.output_tokens);
                    result.cache_read_input_tokens =
                        u.cache_read_input_tokens.or(result.cache_read_input_tokens);
                }
            }
            Some("message_delta") => {
                if let Some(u) = &evt.usage {
                    if u.input_tokens.is_some() {
                        result.input_tokens = u.input_tokens;
                    }
                    if u.output_tokens.is_some() {
                        result.output_tokens = u.output_tokens;
                    }
                    if u.cache_read_input_tokens.is_some() {
                        result.cache_read_input_tokens = u.cache_read_input_tokens;
                    }
                }
            }
            _ => {}
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sse_data_valid() {
        assert_eq!(
            parse_sse_data("data: {\"key\": true}"),
            Some("{\"key\": true}")
        );
    }

    #[test]
    fn parse_sse_data_empty() {
        assert_eq!(parse_sse_data("data: "), None);
    }

    #[test]
    fn parse_sse_data_no_prefix() {
        assert_eq!(parse_sse_data("event: ping"), None);
    }

    #[test]
    fn extract_usage_from_message_start_and_delta() {
        let sse = "\
event: message_start\n\
data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":100,\"output_tokens\":0}}}\n\
\n\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"hello\"}}\n\
\n\
event: message_delta\n\
data: {\"type\":\"message_delta\",\"usage\":{\"output_tokens\":50}}\n";

        let usage = extract_stream_usage(sse);
        assert_eq!(usage.input_tokens, Some(100));
        assert_eq!(usage.output_tokens, Some(50));
        assert!(usage.error.is_none());
    }

    #[test]
    fn extract_usage_detects_error() {
        let sse = "\
event: error\n\
data: {\"error\":{\"code\":\"rate_limit\",\"message\":\"too many requests\"},\"request_id\":\"req_123\"}\n";

        let usage = extract_stream_usage(sse);
        assert_eq!(
            usage.error,
            Some("[rate_limit] too many requests".to_string())
        );
    }

    #[test]
    fn extract_usage_empty_input() {
        let usage = extract_stream_usage("");
        assert!(usage.input_tokens.is_none());
        assert!(usage.output_tokens.is_none());
        assert!(usage.error.is_none());
    }
}
