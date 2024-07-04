use async_trait::async_trait;

use api::Visibility;

use crate::service::*;

pub mod msg;
pub mod svc;

#[async_trait]
trait ESCacheService: ESInner {
    async fn clear_all_caches(&self, resp: ESMResp<()>) -> anyhow::Result<()>;

    async fn get_image_visibility(&self, resp: ESMResp<Visibility>, image: String) -> anyhow::Result<()>;
}
