pub mod album;
pub mod group;
pub mod library;
pub mod media;
pub mod ticket;
pub mod user;

#[macro_export]
macro_rules! message {
    ($name:ident, $target:tt) => {
        paste::paste!{
            pub async fn [<$name:snake>](req: &[<$name:camel Req>]) -> anyhow::Result<[<$name:camel Resp>]> {
                let resp = gloo_net::http::Request::post(format!("/entanglement/api/{}", stringify!([<$target:lower>])).as_str())
                    .json(&[<$target:camel Message>]::[<$name:camel>](req.clone()))?
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
