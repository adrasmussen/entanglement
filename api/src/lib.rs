use std::fmt::{Formatter, Display};
use std::sync::Arc;

pub mod album;
pub mod auth;
pub mod comment;
pub mod library;
pub mod media;
pub mod task;

pub const ORIGINAL_PATH: &str = "originals";
pub const THUMBNAIL_PATH: &str = "thumbnails";
pub const SLICE_PATH: &str = "slices";

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
