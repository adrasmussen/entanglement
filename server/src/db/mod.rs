use common::db::DbBackend;

use crate::service::ESInner;
use api::media::MediaUuid;

pub mod msg;
pub mod svc;

// these are the database RPC calls that any backend server must be able to process
trait ESDbRunner: ESInner + DbBackend {
    async fn clear_access_cache(&self, media_uuid: Vec<MediaUuid>) -> anyhow::Result<()>;
}
