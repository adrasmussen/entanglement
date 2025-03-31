use std::{path::PathBuf, sync::Arc};

use anyhow::Result;
use axum::{
    body::Body,
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use futures::StreamExt;
use http::{
    header::{ACCEPT_RANGES, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, RANGE},
    HeaderMap, HeaderValue,
};
use mime_guess::MimeGuess;
use tokio::{
    fs::{read_link, File},
    io::AsyncSeekExt,
};
use tokio_util::codec::{BytesCodec, FramedRead};
use tracing::{debug, instrument, warn};

use crate::{
    auth::check::AuthCheck,
    http::{auth::CurrentUser, svc::HttpEndpoint, AppError},
};
use api::{media::MediaUuid, ORIGINAL_PATH};

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
    headers: HeaderMap,
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Path((dir, media_uuid)): Path<(String, MediaUuid)>,
) -> Result<Response, AppError> {
    debug!("serving media");

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

    // we only ever serve out of the linking directory, since we control its organization
    let filename = state.config.media_srvdir.join(dir).join(&media_uuid);

    let mut file_handle = match File::open(&filename).await {
        Ok(f) => f,
        Err(err) => return Ok((StatusCode::NOT_FOUND, err.to_string()).into_response()),
    };

    let file_metadata = file_handle.metadata().await?;

    let length: i64 = file_metadata.len().try_into()?;

    let (partial, (start, end)) = match headers.get(RANGE) {
        None => (false, (0, length)),
        Some(val) => (
            true,
            match parse_ranges(state.clone(), val.to_str()?, length) {
                Ok(v) => v,
                Err(err) => {
                    return Ok((StatusCode::RANGE_NOT_SATISFIABLE, format!("{err}")).into_response())
                }
            },
        ),
    };

    // for some reason, my testing environment doesn't want to send this header, so i
    // can't yet test it.  we could assume media is immutable, but that isn't strictly
    // enforceable from within entanglement
    // if let Some(mtime) = headers.get(IF_MODIFIED_SINCE) {...}

    // response headers
    //
    // while modern browsers can get by without any of these being set, we need them all
    // to be correct so that streaming works
    let mut headers = HeaderMap::new();

    // tells the client that they can send Ranges back
    headers.insert(ACCEPT_RANGES, HeaderValue::from_static("bytes"));

    // client uses this to figure out where the stream ends
    headers.insert(CONTENT_LENGTH, HeaderValue::from(end - start));

    if partial {
        headers.insert(
            CONTENT_RANGE,
            HeaderValue::from_str(&format!("bytes {start}-{}/{length}", end - 1))?,
        );
    }

    // make sure we echo the range
    // headers.insert(CONTENT_RANGE, ...)

    // follow the symlnk to (maybe) fetch the mime type of the media based on the
    // file extention of the original
    match MimeGuess::from_path(
        read_link(
            state
                .config
                .media_srvdir
                .join(ORIGINAL_PATH)
                .join(media_uuid),
        )
        .await?,
    )
    .first()
    {
        Some(mime) => {
            headers.insert(CONTENT_TYPE, HeaderValue::from_str(mime.essence_str())?);
        }
        None => {
            warn!("failed to guess mime type")
        }
    }

    // cache controls?
    // headers.insert(CACHE_CONTROL, ...)

    let body = if partial {
        file_handle.seek(std::io::SeekFrom::Current(start)).await?;
        Body::from_stream(
            FramedRead::new(file_handle, BytesCodec::new()).take((end - start).try_into()?),
        )
    } else {
        Body::from_stream(FramedRead::new(file_handle, BytesCodec::new()))
    };

    let code = if partial {
        StatusCode::PARTIAL_CONTENT
    } else {
        StatusCode::OK
    };

    Ok((code, headers, body).into_response())
}

// logic copied from https://github.com/dicej/tagger/blob/master/server/src/media.rs
//
// the http::response module does not seem to have an easy way to concatenate reponses
// in way that would be compatible with the http spec
fn parse_ranges(state: Arc<HttpEndpoint>, ranges: &str, length: i64) -> Result<(i64, i64)> {
    if !ranges.starts_with("bytes=") {
        return Err(anyhow::Error::msg("invalid range unit"));
    }

    let regex = state.range_regex.clone();

    let mut match_iter = regex
        .captures_iter(ranges)
        .map(|c| c.extract::<2>())
        .map(|(_, [s, e])| parse_endpoints(s, e));

    let (start, end) = match match_iter.next() {
        None => return Ok((0, length)),
        Some(range) => {
            let range = range?;

            match range {
                (Some(start), Some(end)) => (start, end + 1),
                (Some(start), None) => (start, length),
                (None, Some(end)) => (length - end, length),
                (None, None) => (0, length),
            }
        }
    };

    if start > length || end > length || start > end || end <= 0 || start <= 0 {
        return Err(anyhow::Error::msg("invalid range"));
    }

    if let Some(_) = match_iter.next() {
        return Err(anyhow::Error::msg("multiple ranges unsupported"));
    }

    Ok((start, end))
}

fn parse_endpoints(start: &str, end: &str) -> Result<(Option<i64>, Option<i64>)> {
    let parse = |s| match s {
        "" => Ok(None),
        s => Some(
            s.parse::<i64>()
                .map_err(|_| anyhow::Error::msg("failed to parse endpoint")),
        )
        .transpose(),
    };

    Ok((parse(start)?, parse(end)?))
}
