use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use std::sync::Arc;

pub mod auth;
pub mod collection;
pub mod comment;
pub mod library;
pub mod log;
pub mod media;
pub mod search;
pub mod task;

pub const ORIGINAL_PATH: &str = "originals";
pub const THUMBNAIL_PATH: &str = "thumbnails";
pub const SLICE_PATH: &str = "slices";

// set folding
//
// some databases do not support a column type of set/vec/list, so we need a consistent
// method to convert String <> HashSet.  fixing the folding scheme/separator means that
// we can use substring methods in the database to check for elements in the set.
pub const FOLDING_SEPARATOR: &str = "|";

pub fn fold_set(set: HashSet<String>) -> anyhow::Result<String> {
    if set
        .iter()
        .map(|s| s.contains(FOLDING_SEPARATOR))
        .collect::<HashSet<bool>>()
        .contains(&true)
    {
        return Err(anyhow::Error::msg(format!(
            "invalid character in folded set: '{}'",
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
                .ok_or_else(|| anyhow::Error::msg("folding separator const is zero length"))?,
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

#[derive(Clone, Debug)]
pub struct WebError(Arc<anyhow::Error>);

impl WebError {
    pub fn new() -> Self {
        WebError(Arc::new(anyhow::Error::msg("")))
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

// TODO -- this needs to be modified for the reverse proxy, too
#[macro_export]
macro_rules! endpoint {
    ($name:ident) => {
        paste::paste!{
            pub async fn [<$name:snake>](req: &[<$name:camel Req>]) -> Result<[<$name:camel Resp>], crate::WebError> {
                let resp = gloo_net::http::Request::post(format!("/entanglement/api/{}", stringify!([<$name:camel>])).as_str())
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
