pub mod album;
pub mod comment;
pub mod library;
pub mod media;

pub const THUMBNAIL_PATH: &str = "thumbnails";
pub const SLICE_PATH: &str = "slices";

#[macro_export]
macro_rules! endpoint {
    ($name:ident) => {
        paste::paste!{
            pub async fn [<$name:snake>](req: &[<$name:camel Req>]) -> anyhow::Result<[<$name:camel Resp>]> {
                let resp = gloo_net::http::Request::post(format!("/entanglement/api/{}", stringify!([<$name:camel>])).as_str())
                    .json(&req.clone())?
                    .send()
                    .await?;

                if resp.ok() {
                    Ok(resp.json().await?)
                } else {
                    Err(anyhow::Error::msg(resp.text().await?))
                }
            }
        }
    };
}
