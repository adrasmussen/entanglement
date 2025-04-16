use std::{io::SeekFrom, path::PathBuf, sync::Arc};

use anyhow::Result;
use axum::{
    body::Body,
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use http::{
    header::{ACCEPT_RANGES, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, RANGE},
    HeaderMap, HeaderValue,
};
use mime_guess::MimeGuess;
use tokio::{
    fs::{read_link, File},
    io::AsyncSeekExt,
};
use tokio_stream::StreamExt;
use tokio_util::codec::{BytesCodec, FramedRead};
use tracing::{debug, instrument, warn};

use crate::{
    auth::check::AuthCheck,
    http::{auth::CurrentUser, svc::HttpEndpoint, AppError},
};
use api::{media::MediaUuid, ORIGINAL_PATH};

// media stream/download
//
// this is the core media downloading function through which all media accesses happen
//
// as such, it has to enforce the authorization model, but it also has to include all
// of the http streaming logic (range, mime, etc) that depends on the filesystem
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

    // the layout of the srv directory is completely controlled by the server,
    // so we could move this around as much as we wanted for better control
    //
    // dir is usually one of several constants, and could hypothetically be
    // an enum or similar if there end up being too many variants
    //
    // see also task/scan_utils.rs for the functions that control how new media
    // is added to the streaming directories.
    let filename = state.config.fs.media_srvdir.join(dir).join(&media_uuid);

    // here and below we use tokio logic to handle the filesystem operations
    // so that we don't block the server threads
    let mut file_handle = match File::open(&filename).await {
        Ok(f) => f,
        Err(err) => return Ok((StatusCode::NOT_FOUND, err.to_string()).into_response()),
    };

    let file_metadata = file_handle.metadata().await?;

    let length = file_metadata.len();

    // range header check
    //
    // this is the header that allows for seeking through videos, pause/resuming downloads,
    // and all of the other magic associated with streaming media.  it works by specifying
    // a byte range (with rather complicated rules, see below) which are then sent with a
    // verification header back to the client
    //
    // without this logic, browsers will have to buffer the whole file before they can seek.
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

    // http response headers
    //
    // while modern browsers can get by without any of these being set, we need them all
    // to be correct so that streaming works
    let mut headers = HeaderMap::new();

    // tells the client that they can send Ranges back
    headers.insert(ACCEPT_RANGES, HeaderValue::from_static("bytes"));

    // client uses this to figure out where the stream ends
    headers.insert(CONTENT_LENGTH, HeaderValue::from(end - start));

    // see below for comments on the semantics surrounding start and end -- effectively,
    // start is zero-indexed but end is one-indexed, but "s-e" are both zero-indexed
    if partial {
        headers.insert(
            CONTENT_RANGE,
            HeaderValue::from_str(&format!("bytes {start}-{}/{length}", end - 1))?,
        );
    }

    // follow the symlnk to (maybe) fetch the mime type of the media based on the
    // file extention of the original
    match MimeGuess::from_path(
        read_link(
            state
                .config
                .fs
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

    // for some reason, my testing environment doesn't want to send this header, so i
    // can't yet test it.  we could assume media is immutable, but that isn't strictly
    // enforceable from within entanglement
    // if let Some(mtime) = headers.get(IF_MODIFIED_SINCE) {...}

    // cache controls?
    // headers.insert(CACHE_CONTROL, ...)

    // http response body
    //
    // starting with the file handle, we first use tokio's AsyncRead to create a FramedRead,
    // which is an adapter for AsyncRead -> Stream that uses a codec to define how much of
    // the underlying structure is returned on each call to the Stream's poll_next().
    //
    // in this case, we want a Byte each time, so the codec is very simple.  the Stream is
    // then fed into axum's built-in streaming body logic.
    let body = if partial {
        // if we want the bytes from partway into the file, we need to first move the Seek
        // pointer to the starting byte
        file_handle
            .seek(SeekFrom::Current(start.try_into()?))
            .await?;

        // then, we create a new stream that only has (end - start) bytes and use that
        //
        // note the argument to take() is one-indexed; see below for semantics
        Body::from_stream(
            FramedRead::new(file_handle, BytesCodec::new()).take((end - start).try_into()?),
        )
    } else {
        // for normal reads, just consume the whole file
        Body::from_stream(FramedRead::new(file_handle, BytesCodec::new()))
    };

    // http response status code
    //
    // for readability, we keep this apart from the body/header logic.  in the event that we
    // support multipart/form-data, this will need to be more sophisticated.
    let code = if partial {
        StatusCode::PARTIAL_CONTENT
    } else {
        StatusCode::OK
    };

    Ok((code, headers, body).into_response())
}

// http range header parser
//
// logic copied from https://github.com/dicej/tagger/blob/master/server/src/media.rs
//
// errors here should be reported by caller as StatusCode::RANGE_NOT_SATISFIABLE
//
// if we enable multipart support, this will need to be adapted to produce a vec
fn parse_ranges(state: Arc<HttpEndpoint>, ranges: &str, length: u64) -> Result<(u64, u64)> {
    // there is only one supported unit, but the spec technically allows for others
    if !ranges.starts_with("bytes=") {
        return Err(anyhow::Error::msg("invalid range unit"));
    }

    // the regex should only be created once, and HttpEndpoint is the only place
    let regex = state.range_regex.clone();

    // even though we only support sending a single range back, we need to check to see if
    // the client is expecting more.  the const generic for extract is the number of
    // capture groups, and must match the regex (or the whole thing will panic)
    let mut match_iter = regex
        .captures_iter(ranges)
        .map(|c| c.extract::<2>())
        .map(|(_, [s, e])| parse_endpoints(s, e));

    let (start, end) = match match_iter.next() {
        None => return Ok((0, length)),
        Some(range) => {
            let range = range?;

            // the output (start, end) semantics are awkward
            //
            // start is used in seek(), where 0 indicates "before the first byte."
            // it is zero-indexed.
            //
            // end is used to determine length, where 4 means "read the first four bytes."
            // it is one-indexed.
            //
            // however, both s and e in the "s-e" pattern are zero-indexed, and thus the
            // maximal value of e is length-1
            //
            // the simplest method is to have (end - start) indicate the total length
            // the stream to the one-indexed take() while ensuring that start() remains
            // zero-indexed.  thus, end is about count and start is about position.
            match range {
                // "0-511" => get the first 512 bytes => (end - start) = 512
                //
                // in this pattern only, we need to read the eth byte, so the stopping point
                // is one further than e itself (i.e. convert zero- to one-index)
                (Some(s), Some(e)) => (s, e + 1),
                // "512-" (for 1024b file) => get second 512 => (end - start) = 512
                //
                // the difference in indexes automatically picks up the missing byte from
                // starting at the beginning of the sth byte
                (Some(s), None) => (s, length),
                // "-512" (for 1024b file) => get 512b leading to end => (end - start) = 512
                //
                // the difference in indexes is accounted for because e is going backwards
                (None, Some(e)) => (length - e, length),
                // if the range header is set but does not specify a by range, return the
                // whole file to the client
                (None, None) => (0, length),
            }
        }
    };

    // sanity checks -- note that u64 cannot be negative so we just need to assert order
    // and that we will send something back
    if start > length || end > length || start > end || end == 0 {
        return Err(anyhow::Error::msg("invalid range"));
    }

    // this avoids a collect() and a heap allocation of a Vec, but it's probably not a
    // relevant cost compared to the rest of the system
    if let Some(_) = match_iter.next() {
        return Err(anyhow::Error::msg("multiple ranges unsupported"));
    }

    Ok((start, end))
}

fn parse_endpoints(start: &str, end: &str) -> Result<(Option<u64>, Option<u64>)> {
    let parse = |s| match s {
        "" => Ok(None),
        s => Some(
            s.parse::<u64>()
                .map_err(|_| anyhow::Error::msg("failed to parse endpoint")),
        )
        .transpose(),
    };

    Ok((parse(start)?, parse(end)?))
}
