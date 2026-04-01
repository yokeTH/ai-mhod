use axum::Json;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub data: T,
    pub pagination: Option<Pagination>,
}

#[derive(Debug, Serialize)]
pub struct Pagination {
    pub page: u32,
    pub last: u32,
    pub limit: u32,
    pub total: u64,
}

impl<T: Serialize> ApiResponse<T> {
    #[must_use]
    pub fn ok(data: T) -> Self {
        Self {
            data,
            pagination: None,
        }
    }
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> Response {
        Json(self).into_response()
    }
}
