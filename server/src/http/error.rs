use axum::{
    extract::{Extension, Json, Path, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};

use serde::Serialize;

use crate::http::HttpError;
use crate::service::ESError;

// https://github.com/tokio-rs/axum/blob/main/examples/error-handling/src/main.rs

impl IntoResponse for ESError {
    fn into_response(self) -> Response {
        #[derive(Serialize)]
        struct ErrorResponse {
            message: String,
        }

        let (status, message) = match self {
            ESError::Http(err) => match err {
                HttpError::JsonRejection(rejection) => {
                    (rejection.status(), rejection.body_text())
                }
            }
        };

        (status, Json(ErrorResponse { message })).into_response()
    }
}
