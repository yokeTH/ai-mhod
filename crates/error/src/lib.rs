use axum::Json;
use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use thiserror::Error;

/// Ordered by HTTP status code: 400 → 401 → 403 → 404 → 409 → 500
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub enum ErrorCode {
    // 400
    InvalidInput,
    // 401
    Unauthorized,
    InvalidToken,
    InvalidClaims,
    BadSignature,
    // 403
    Forbidden,
    // 404
    NotFound,
    // 409
    AlreadyExists,
    Conflict,
    // 422
    DuplicateEntry,
    // 500
    InternalError,
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AppError {
    // 400
    #[error("{0}")]
    InvalidInput(String),

    // 401
    #[error("{0}")]
    Unauthorized(String),

    #[error("invalid token")]
    InvalidToken,

    #[error("invalid claims")]
    InvalidClaims,

    #[error("bad signature")]
    BadSignature,

    // 403
    #[error("{0}")]
    Forbidden(String),

    // 404
    #[error("{0}")]
    NotFound(String),

    // 409
    #[error("{0}")]
    AlreadyExists(String),

    #[error("{0}")]
    Conflict(String),

    // 422
    #[error("{0}")]
    DuplicateEntry(String),

    // 500
    #[error("an internal server error occurred")]
    Anyhow(#[from] anyhow::Error),
}

impl AppError {
    #[must_use]
    pub fn invalid_input(msg: impl Into<String>) -> Self {
        Self::InvalidInput(msg.into())
    }

    #[must_use]
    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self::Unauthorized(msg.into())
    }

    #[must_use]
    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::Forbidden(msg.into())
    }

    #[must_use]
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }

    #[must_use]
    pub fn already_exists(msg: impl Into<String>) -> Self {
        Self::AlreadyExists(msg.into())
    }

    #[must_use]
    pub fn conflict(msg: impl Into<String>) -> Self {
        Self::Conflict(msg.into())
    }

    #[must_use]
    pub fn duplicate_entry(msg: impl Into<String>) -> Self {
        Self::DuplicateEntry(msg.into())
    }

    fn status_code(&self) -> StatusCode {
        match self {
            Self::InvalidInput(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized(_)
            | Self::InvalidToken
            | Self::InvalidClaims
            | Self::BadSignature => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::AlreadyExists(_) | Self::Conflict(_) => StatusCode::CONFLICT,
            Self::DuplicateEntry(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Anyhow(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_code(&self) -> ErrorCode {
        match self {
            Self::InvalidInput(_) => ErrorCode::InvalidInput,
            Self::Unauthorized(_) => ErrorCode::Unauthorized,
            Self::InvalidToken => ErrorCode::InvalidToken,
            Self::InvalidClaims => ErrorCode::InvalidClaims,
            Self::BadSignature => ErrorCode::BadSignature,
            Self::Forbidden(_) => ErrorCode::Forbidden,
            Self::NotFound(_) => ErrorCode::NotFound,
            Self::AlreadyExists(_) => ErrorCode::AlreadyExists,
            Self::Conflict(_) => ErrorCode::Conflict,
            Self::DuplicateEntry(_) => ErrorCode::DuplicateEntry,
            Self::Anyhow(_) => ErrorCode::InternalError,
        }
    }
}

#[derive(Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ErrorResponse {
    pub error: ErrorData,
}

#[derive(Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ErrorData {
    pub code: ErrorCode,
    pub message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let code = self.error_code();

        if let Self::Anyhow(ref err) = self {
            tracing::error!(error = ?err, "internal server error");
        }

        let message = match &self {
            Self::Anyhow(_) => "an internal server error occurred".to_string(),
            _ => self.to_string(),
        };

        (
            status,
            Json(ErrorResponse {
                error: ErrorData { code, message },
            }),
        )
            .into_response()
    }
}

impl From<JsonRejection> for AppError {
    fn from(rejection: JsonRejection) -> Self {
        Self::InvalidInput(rejection.body_text())
    }
}

impl From<QueryRejection> for AppError {
    fn from(rejection: QueryRejection) -> Self {
        Self::InvalidInput(rejection.body_text())
    }
}

impl From<PathRejection> for AppError {
    fn from(rejection: PathRejection) -> Self {
        Self::InvalidInput(rejection.body_text())
    }
}

/// Proxy-specific error that returns Anthropic-style JSON responses.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ProxyError {
    #[error("authentication failed: invalid or missing API key")]
    Unauthorized,
    #[error("upstream request failed: {0}")]
    UpstreamError(String),
    #[error("{0}")]
    BadRequest(String),
    #[error("failed to obtain upstream auth token: {0}")]
    TokenError(String),
}

impl IntoResponse for ProxyError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            ProxyError::Unauthorized => (StatusCode::UNAUTHORIZED, self.to_string()),
            ProxyError::UpstreamError(msg) => (StatusCode::BAD_GATEWAY, msg.clone()),
            ProxyError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ProxyError::TokenError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        };

        tracing::warn!(error = %self, "Request failed");

        (
            status,
            Json(serde_json::json!({
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

pub type Result<T> = std::result::Result<T, AppError>;
