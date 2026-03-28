use actix_web::{HttpResponse, ResponseError};
use firebox_core::CoreError;

use crate::dto::ErrorResponse;

/// Newtype wrapper so we can implement ResponseError (orphan rule).
pub struct ApiError(pub CoreError);

impl std::fmt::Debug for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<CoreError> for ApiError {
    fn from(e: CoreError) -> Self {
        ApiError(e)
    }
}

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        let body = ErrorResponse { error: self.0.to_string() };
        match &self.0 {
            CoreError::NotFound(_)   => HttpResponse::NotFound().json(body),
            CoreError::Conflict(_)   => HttpResponse::Conflict().json(body),
            CoreError::Validation(_) => HttpResponse::BadRequest().json(body),
            _                        => HttpResponse::InternalServerError().json(body),
        }
    }
}
