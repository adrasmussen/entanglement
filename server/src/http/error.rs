use axum::{
    extract::{rejection::JsonRejection, Extension, Json, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};

use serde::Serialize;

use crate::service::*;

// https://github.com/tokio-rs/axum/blob/main/examples/error-handling/src/main.rs

impl IntoResponse for ESError {
    fn into_response(self) -> Response {
        #[derive(Serialize)]
        struct ErrorResponse {
            message: String,
        }

        let (status, message) = match self {
            ESError::AnyhowError(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                err.to_string(),
            ),
            ESError::ChannelSendError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal communications error".to_string(),
            ),
            ESError::ChannelRecvError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal communications error".to_string(),
            ),
            ESError::JsonRejection(rejection) => (rejection.status(), rejection.body_text()),
        };

        (status, Json(ErrorResponse { message })).into_response()
    }
}
