use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;

use anyhow;

use async_trait::async_trait;

use futures::TryStreamExt;

use crate::service::ESInner;
use api::{album::*, group::*, image::*, library::*, ticket::*, user::*, *};

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

    // media functions
    async fn add_image(&self, media: Media) -> anyhow::Result<MediaUuid>;

    async fn get_image(&self, uuid: MediaUuid) -> anyhow::Result<Media>;

    async fn update_media(
        &self,
        user: String,
        uuid: MediaUuid,
        change: MediaMetadata,
    ) -> anyhow::Result<()>;

    async fn search_media(
        &self,
        user: String,
        filter: String,
    ) -> anyhow::Result<HashMap<MediaUuid, Image>>;

    // album functions
    async fn add_album(&self, album: Album) -> anyhow::Result<()>;

    async fn get_album(&self, uuid: AlbumUuid) -> anyhow::Result<Album>;

    async fn update_album(
        &self,
        uuid: AlbumUuid,
        change: AlbumMetadata,
    ) -> anyhow::Result<()>;

    async fn search_albums(&self, user: String, filter: String) -> anyhow::Result<Vec<AlbumUuid>>;

    async fn search_media_in_album(
        &self,
        user: String,
        uuid: AlbumUuid,
        filter: String,
    ) -> anyhow::Result<Vec<MediaUuid>>;

    // library functions
    async fn add_library(&self, library: Library) -> anyhow::Result<LibraryUuid>;

    async fn get_library(&self, uuid: LibraryUuid) -> anyhow::Result<Library>;

    async fn search_media_in_library(
        &self,
        user: String,
        uuid: LibraryUuid,
        filter: String,
        hidden: bool,
    ) -> anyhow::Result<Vec<MediaUuid>>;

    // ticket functions
    async fn create_ticket(&self, ticket: Ticket) -> anyhow::Result<TicketUuid>;

    async fn create_comment(&self, comment: TicketComment) -> anyhow::Result<CommentUuid>;

    async fn get_ticket(&self, ticket_uuid: TicketUuid) -> anyhow::Result<Ticket>;

    async fn search_tickets(
        &self,
        user: String,
        filter: String,
        resolved: bool,
    ) -> anyhow::Result<Vec<TicketUuid>>;
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
