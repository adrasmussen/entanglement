use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;

use anyhow;

use async_trait::async_trait;

use futures::TryStreamExt;

use crate::service::ESInner;
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
    async fn add_user(&self, user: User) -> anyhow::Result<()>;

    async fn get_user(&self, uid: String) -> anyhow::Result<User>;

    async fn delete_user(&self, uid: String) -> anyhow::Result<()>;

    async fn add_group(&self, group: Group) -> anyhow::Result<()>;

    async fn get_group(&self, gid: String) -> anyhow::Result<Group>;

    async fn delete_group(&self, gid: String) -> anyhow::Result<()>;

    async fn add_user_to_group(&self, uid: String, gid: String) -> anyhow::Result<()>;

    async fn rm_user_from_group(&self, uid: String, gid: String) -> anyhow::Result<()>;

    // image functions
    async fn add_image(&self, image: Image) -> anyhow::Result<ImageUuid>;

    async fn get_image(&self, uuid: ImageUuid) -> anyhow::Result<Image>;

    async fn update_image(
        &self,
        user: String,
        uuid: ImageUuid,
        change: ImageMetadata,
    ) -> anyhow::Result<()>;

    async fn filter_images(
        &self,
        user: String,
        filter: String,
    ) -> anyhow::Result<HashMap<ImageUuid, Image>>;

    // album functions
    async fn add_album(&self, album: Album) -> anyhow::Result<()>;

    async fn get_album(&self, uuid: AlbumUuid) -> anyhow::Result<Album>;

    async fn update_album(
        &self,
        user: String,
        uuid: AlbumUuid,
        change: AlbumMetadata,
    ) -> anyhow::Result<()>;

    async fn filter_albums(&self, user: String, filter: String) -> anyhow::Result<()>;

    // library functions
    async fn add_library(&self, library: Library) -> anyhow::Result<()>;

    async fn get_library(&self, uuid: LibraryUuid) -> anyhow::Result<Library>;

    async fn update_library(
        &self,
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
