use std::error::Error;

use anyhow;

use async_trait::async_trait;

use futures::TryStreamExt;

pub mod msg;
pub mod query;

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
