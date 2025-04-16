use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use std::sync::Arc;

pub mod auth;
pub mod collection;
pub mod comment;
pub mod library;
pub mod media;
pub mod search;
pub mod task;

// filesystem/http paths
//
// these paths are used to control where under the media_srvdir the server will create
// symlinks, thumbnails, and so on.  since the /media route on the http server points
// to the media_srvdir directory, these also control the urls used for downloading.
pub const ORIGINAL_PATH: &str = "originals";
pub const THUMBNAIL_PATH: &str = "thumbnails";
pub const SLICE_PATH: &str = "slices";

// http url root
//
// until we figure out how to have dioxus dynamically fetch the revese proxy settings
// from the server at runtime, we have to set this constant here AND in Dioxus.toml.
//
// when running behind a reverse proxy, the upstream settings must also match.
pub const HTTP_URL_ROOT: &str = "entanglement";

// set folding
//
// some databases do not support a column type of set/vec/list, so we need a consistent
// method to convert String <> HashSet.  fixing the folding scheme/separator means that
// we can use substring methods in the database to check for elements in the set.
//
// these methods are also used in the webapp, which adds the awkward requirement that
// the separator be commonly-found on keyboards.  eventually, better text input methods
// could remove this requirement.
pub const FOLDING_SEPARATOR: &str = "|";

pub fn fold_set(set: HashSet<String>) -> anyhow::Result<String> {
    if set
        .iter()
        .map(|s| s.contains(FOLDING_SEPARATOR))
        .collect::<HashSet<bool>>()
        .contains(&true)
    {
        return Err(anyhow::Error::msg(format!(
            "internal error: invalid character in folded set '{}'",
            FOLDING_SEPARATOR
        )));
    }

    Ok(set
        .iter()
        .fold(String::new(), |a, b| a + b + FOLDING_SEPARATOR)
        .trim_matches(
            FOLDING_SEPARATOR
                .chars()
                .next()
                .ok_or_else(|| anyhow::Error::msg("internal error: folding separator const is zero length"))?,
        )
        .to_string())
}

pub fn unfold_set(str: &str) -> HashSet<String> {
    let mut set = str
        .split(FOLDING_SEPARATOR)
        .map(|s| s.to_string())
        .collect::<HashSet<String>>();

    set.retain(|s| !s.is_empty());

    set
}

// weberror
//
// anyhow::Error does not implement serde::de::StdError, which prevents it from being used
// in Dioxus's ErrorBoundary handle_error logic.  thus, we create this mostly-transparent
// wrapper and connect it to both anyhow and the gloo_net errors returned by the api calls
#[derive(Clone, Debug)]
pub struct WebError(Arc<anyhow::Error>);

impl WebError {
    pub fn new() -> Self {
        WebError(Arc::new(anyhow::Error::msg("")))
    }

    pub fn msg(msg: String) -> Self {
        WebError(Arc::new(anyhow::Error::msg(msg)))
    }
}

impl Display for WebError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl serde::de::StdError for WebError {}

impl From<gloo_net::Error> for WebError {
    fn from(value: gloo_net::Error) -> Self {
        WebError(Arc::new(value.into()))
    }
}

impl From<anyhow::Error> for WebError {
    fn from(value: anyhow::Error) -> Self {
        WebError(Arc::new(value))
    }
}

// endpoints
//
// these functions control how the webapp communicates with the server, either by
// create the future directly or by providing a String that is interpreted by the
// browser (img or a tags)
#[macro_export]
macro_rules! endpoint {
    ($name:ident) => {
        paste::paste!{
            pub async fn [<$name:snake>](req: &[<$name:camel Req>]) -> Result<[<$name:camel Resp>], crate::WebError> {
                use crate::HTTP_URL_ROOT;
                let resp = gloo_net::http::Request::post(format!("/{}/api/{}", HTTP_URL_ROOT, stringify!([<$name:camel>])).as_str())
                    .json(&req.clone())?
                    .send()
                    .await?;

                if resp.ok() {
                    Ok(resp.json().await?)
                } else {
                    Err(anyhow::Error::msg(resp.text().await?).into())
                }
            }
        }
    };
}

pub fn full_link(media_uuid: media::MediaUuid) -> String {
    format!("/{HTTP_URL_ROOT}/media/{ORIGINAL_PATH}/{media_uuid}")
}

pub fn thumbnail_link(media_uuid: media::MediaUuid) -> String {
    format!("/{HTTP_URL_ROOT}/media/{THUMBNAIL_PATH}/{media_uuid}")
}
