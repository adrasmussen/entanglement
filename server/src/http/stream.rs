use std::{path::PathBuf, sync::Arc};

use axum::{
    extract::{Extension, Path, Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use http::{
    header::{ACCEPT_RANGES, CONTENT_TYPE},
    HeaderMap, HeaderValue,
};
use mime_guess::MimeGuess;
use tokio::fs::read_link;
use tokio_util::io::ReaderStream;
use tracing::{debug, error, info, instrument};

use crate::{
    auth::check::AuthCheck,
    http::{auth::CurrentUser, svc::HttpEndpoint, AppError},
};
use api::media::MediaUuid;

// http handlers

// note that all of these handlers return an AppError, which is a transparent
// wrapper around anyhow::Error so that we can impl IntoResponse
//
// this lets us use ? and ultimately send any errors all the way back to the
// caller, which simplifies everything drastically

// we use an axum Extension extractor to grab the CurrentUser struct inserted
// by the middleware auth layer; see http/auth.rs

// this relatively straightforward reader handles all types of media (images,
// thumbnails, video, etc), which will likely not work once we need video slices
#[instrument(skip_all)]
pub(super) async fn stream_media(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Path((dir, media_uuid)): Path<(String, MediaUuid)>,
) -> Result<Response, AppError> {
    debug!({dir = dir, media_uuid = media_uuid}, "streaming media");

    let state = state.clone();
    let uid = current_user.uid.clone();

    if !state.can_access_media(&uid, &media_uuid).await? {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let media_uuid = media_uuid.to_string();

    // the pathbuf join() method inexplicably replaces the path if the argument
    // is absolute, so we include this explicit check
    if PathBuf::from(&dir).is_absolute() || PathBuf::from(&media_uuid).is_absolute() {
        return Ok(StatusCode::BAD_REQUEST.into_response());
    }

    // parse Ranges here

    // we only ever serve out of the linking directory, since we control its organization
    let filename = state.config.media_srvdir.join(dir).join(media_uuid);

    let file_handle = match tokio::fs::File::open(&filename).await {
        Ok(f) => f,
        Err(err) => return Ok((StatusCode::NOT_FOUND, err.to_string()).into_response()),
    };

    let reader_stream = ReaderStream::new(file_handle);

    // TODO -- use mime_guess to follow the symlink and add the correct mime type
    // TODO -- double check the cache controls header/if-modified-since
    let mut headers = HeaderMap::new();

    // this is critical so that the client knows that they can send Ranges back
    headers.insert(ACCEPT_RANGES, HeaderValue::from_static("bytes"));

    // follow the symlnk to (maybe) fetch the mime type of the media based on the
    // file extention of the original
    match read_link(&filename)
        .await?
        .extension()
        .map(|s| MimeGuess::from_path(s).first())
        .flatten()
    {
        Some(mime) => {
            headers.insert(CONTENT_TYPE, HeaderValue::from_str(mime.essence_str())?);
        }
        None => {}
    }

    Ok((headers, axum::body::Body::from_stream(reader_stream)).into_response())
}
