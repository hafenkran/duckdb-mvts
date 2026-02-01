use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use crate::store::errors::StoreError;

/// Validation error for invalid table references
#[derive(Debug)]
pub struct InvalidTableReference {
    pub message: String,
}

impl std::fmt::Display for InvalidTableReference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for InvalidTableReference {}

impl IntoResponse for InvalidTableReference {
    fn into_response(self) -> Response {
        (StatusCode::BAD_REQUEST, self.message).into_response()
    }
}

/// Helper function to convert StoreError to HTTP response
pub fn store_error_to_response(e: StoreError) -> Response {
    let (status, message) = match e {
        StoreError::TableNotFound { .. } => (StatusCode::NOT_FOUND, e.to_string()),
        StoreError::NoGeometryColumn { .. } => (StatusCode::BAD_REQUEST, e.to_string()),
        StoreError::DatabaseError { .. } => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    (status, message).into_response()
}

