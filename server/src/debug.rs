use std::time::Duration;

use anyhow::Result;
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use tokio::time::sleep;
use tracing::{info, instrument};

use crate::http::auth::CurrentUser;
use api::library::LibraryUuid;

// task debugging task
#[instrument]
pub async fn sleep_task(library_uuid: LibraryUuid) -> Result<i64> {
    info!("info from task");
    sleep(Duration::from_secs(100)).await;

    Ok(-1)
}

// auth bypass middleware
pub async fn bypass_auth(
    State(state): State<String>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    req.extensions_mut().insert(CurrentUser {
        uid: String::from("alex"),
    });

    Ok(next.run(req).await)
}
