use std::sync::Arc;
use std::error::Error;

use anyhow;

use async_trait::async_trait;

use futures::TryStreamExt;

use crate::service::{ESMResp, EntanglementService};

pub mod msg;
pub mod query;
pub mod svc;

// marker trait to allow specific implementations of the ESDbQuery
trait ESDbConn {}

impl ESDbConn for mysql_async::Conn {}

#[async_trait]
trait ESDbQuery<T: ESDbConn> {
    type QueryOutput;

    async fn result_stream(
        self,
        conn: T,
    ) -> anyhow::Result<Option<impl TryStreamExt<Item = Result<Self::QueryOutput, impl Error>>>>;
}

#[async_trait]
trait ESDbService: EntanglementService {
    async fn get_filtered_images(self: Arc<Self>, resp: ESMResp<()>, user: String, filter: String) -> anyhow::Result<()>;

    async fn edit_album(self: Arc<Self>, resp: ESMResp<()>, user: String, album: String, data: ()) -> anyhow::Result<()>;
}
