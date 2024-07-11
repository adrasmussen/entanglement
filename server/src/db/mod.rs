use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;

use anyhow;

use async_trait::async_trait;

use futures::TryStreamExt;

use crate::service::{ESInner, ESMResp};
use api::{auth::*, image::*};

pub mod msg;
pub mod mysql;
pub mod svc;

// these are the database RPC calls that any backend server must be able to process
//
// note that the response to the caller is in the ESMResp, and that the actual return
// of the RPC functions are for successfully sending the reponse
#[async_trait]
trait ESDbService: ESInner {
    // authdb functions
    async fn add_user(&self, resp: ESMResp<()>, user: User) -> anyhow::Result<()>;

    async fn get_user(&self, resp: ESMResp<()>, uid: String) -> anyhow::Result<()>;

    async fn delete_user(&self, resp: ESMResp<()>, uid: String) -> anyhow::Result<()>;

    async fn add_group(&self, resp: ESMResp<()>, group: Group) -> anyhow::Result<()>;

    async fn get_group(&self, resp: ESMResp<()>, gid: String) -> anyhow::Result<()>;

    async fn delete_group(&self, resp: ESMResp<()>, gid: String) -> anyhow::Result<()>;

    async fn add_user_to_group(&self, resp: ESMResp<()>, uid: String, gid: String) -> anyhow::Result<()>;

    async fn rm_user_from_group(&self, resp: ESMResp<()>, uid: String, gid: String) -> anyhow::Result<()>;

    // image functions
    async fn add_image(&self, resp: ESMResp<ImageUuid>, image: Image) -> anyhow::Result<()>;

    async fn get_image(&self, resp: ESMResp<Image>, uuid: ImageUuid) -> anyhow::Result<()>;

    async fn update_image(
        &self,
        resp: ESMResp<()>,
        user: String,
        uuid: ImageUuid,
        change: ImageMetadata,
    ) -> anyhow::Result<()>;

    async fn filter_images(
        &self,
        resp: ESMResp<HashMap<ImageUuid, Image>>,
        user: String,
        filter: String,
    ) -> anyhow::Result<()>;

    // album functions
    async fn add_album(&self, resp: ESMResp<()>, album: Album) -> anyhow::Result<()>;

    async fn get_album(&self, resp: ESMResp<Album>, uuid: AlbumUuid) -> anyhow::Result<()>;

    async fn update_album(
        &self,
        resp: ESMResp<()>,
        user: String,
        uuid: AlbumUuid,
        change: AlbumMetadata,
    ) -> anyhow::Result<()>;

    async fn filter_albums(
        &self,
        resp: ESMResp<()>,
        user: String,
        filter: String,
    ) -> anyhow::Result<()>;

    // library functions
    async fn add_library(&self, resp: ESMResp<()>, library: Library) -> anyhow::Result<()>;

    async fn get_library(&self, resp: ESMResp<Library>, uuid: LibraryUuid) -> anyhow::Result<()>;

    async fn update_library(
        &self,
        resp: ESMResp<()>,
        user: String,
        uuid: LibraryUuid,
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
