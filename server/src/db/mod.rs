use std::error::Error;
use std::sync::Arc;

use anyhow;

use async_trait::async_trait;

use futures::TryStreamExt;

use crate::service::{ESInner, ESMResp};
use api::*;

pub mod msg;
pub mod query;
pub mod svc;
pub mod mysql;

// these are the database RPC calls that any backend server must be able to process
//
// note that the response to the caller is in the ESMResp, and that the actuall return
// of the RPC functions are for successfully sending the reponse
#[async_trait]
trait ESDbService: ESInner {
    async fn add_image(&self, resp: ESMResp<()>, image: Image) -> anyhow::Result<()>;

    async fn get_image(&self, resp: ESMResp<Image>, uuid: ImageUUID) -> anyhow::Result<()>;

    async fn update_image(
        &self,
        resp: ESMResp<()>,
        user: String,
        uuid: ImageUUID,
        change: ImageMetadata,
    ) -> anyhow::Result<()>;

    async fn filter_images(
        &self,
        resp: ESMResp<()>,
        user: String,
        filter: String,
    ) -> anyhow::Result<()>;

    async fn add_album(&self, resp: ESMResp<()>, album: Album) -> anyhow::Result<()>;

    async fn get_album(&self, resp: ESMResp<Album>, uuid: AlbumUUID) -> anyhow::Result<()>;

    async fn update_album(
        &self,
        resp: ESMResp<()>,
        user: String,
        uuid: AlbumUUID,
        change: AlbumMetadata,
    ) -> anyhow::Result<()>;

    async fn filter_albums(
        &self,
        resp: ESMResp<()>,
        user: String,
        filter: String,
    ) -> anyhow::Result<()>;

    async fn add_library(&self, resp: ESMResp<()>, library: Library) -> anyhow::Result<()>;

    async fn get_library(&self, resp: ESMResp<Library>, uuid: LibraryUUID) -> anyhow::Result<()>;

    async fn update_library(
        &self,
        resp: ESMResp<()>,
        user: String,
        uuid: LibraryUUID,
        change: LibraryMetadata,
    ) -> anyhow::Result<()>;
}

// marker trait to allow specific implementations of the ESDbQuery
trait ESDbConn {}

#[async_trait]
trait ESDbQuery<T: ESDbConn> {
    type QueryOutput;

    async fn result_stream(
        self,
        conn: T,
    ) -> anyhow::Result<Option<impl TryStreamExt<Item = Result<Self::QueryOutput, impl Error>>>>;
}
