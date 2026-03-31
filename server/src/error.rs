use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum ProxyError {
    #[error("authentication failed: invalid or missing API key")]
    Unauthorized,
    #[error("upstream request failed: {0}")]
    UpstreamError(String),
    #[error("{0}")]
    BadRequest(String),
}

impl IntoResponse for ProxyError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            ProxyError::Unauthorized => (StatusCode::UNAUTHORIZED, self.to_string()),
            ProxyError::UpstreamError(msg) => (StatusCode::BAD_GATEWAY, msg.clone()),
            ProxyError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
        };

        tracing::error!(error = %self, "Request failed");

        (
            status,
            Json(json!({
                "type": "error",
                "error": {
                    "type": "invalid_request_error",
                    "message": message,
                }
            })),
        )
            .into_response()
    }
}
