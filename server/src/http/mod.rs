use std::sync::Arc;

use async_trait::async_trait;

use axum::{
    extract::{Extension, Json, Path, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};

use crate::http::auth::CurrentUser;
use crate::service::*;
use api::image::FilterImageReq;

pub mod auth;
pub mod error;
pub mod msg;
pub mod svc;

#[async_trait]
pub trait HttpEndpoint: ESInner {
    async fn stream_media(
        State(state): State<Arc<Self>>,
        Extension(current_user): Extension<CurrentUser>,
        Path(path): Path<(String)>,
    ) -> Response;

    async fn search_images(
        State(state): State<Arc<Self>>,
        Extension(current_user): Extension<CurrentUser>,
        json_body: Json<FilterImageReq>,
    ) -> Response;

    // for simplicity, either use an enum and match or read in part of the path

    // edit users
    //
    // add_user
    // get_user
    // delete_user

    // edit groups
    //
    // add_group
    // get_group
    // delete_group
    // add_user_to_group
    // rm_user_from_group

    // albums
    //
    // add_album
    // get_album
    // delete_album
    // add_image_to_album
    // rm_image_from_album

    // library
    //
    // rescan_library

}

#[derive(Debug)]
pub enum HttpError {
    JsonRejection(JsonRejection),
}

impl From<JsonRejection> for HttpError {
    fn from(err: JsonRejection) -> Self {
        Self::JsonRejection(err)
    }
}

impl From<HttpError> for ESError {
    fn from(err: HttpError) -> Self {
        ESError::Http(err)
    }
}
