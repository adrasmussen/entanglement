use std::sync::Arc;
use std::error::Error;

use anyhow;

use async_trait::async_trait;

use futures::TryStreamExt;

use crate::service::{ESInner, ESMResp, EntanglementService, ESM};

pub mod msg;
pub mod query;
pub mod svc;

// these are the database RPC calls that any backend server must be able to process
//
// note that the response to the caller is in the ESMResp, and that the actuall return
// of the RPC functions are for successfully sending the reponse
#[async_trait]
trait ESDbService: ESInner {
    async fn get_filtered_images(&self, resp: ESMResp<()>, user: String, filter: String) -> anyhow::Result<()>;

    async fn edit_album(&self, resp: ESMResp<()>, user: String, album: String, data: ()) -> anyhow::Result<()>;
}

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
