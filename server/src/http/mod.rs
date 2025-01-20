use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

pub mod auth;
pub mod msg;
pub mod svc;

// copied verbatim from https://github.com/tokio-rs/axum/blob/main/examples/anyhow-error-response/src/main.rs

// Make our own error that wraps `anyhow::Error`.
struct AppError(anyhow::Error);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("internal server error: {}", self.0),
        )
            .into_response()
    }
}
// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

// TODO -- define an HTTP endpoint trait, possibly auto-generated from the endpoint!() macros in common::api
