use std::collections::HashSet;
use std::error::Error;

use anyhow;

use async_trait::async_trait;

use futures::TryStreamExt;

use crate::service::ESInner;
use api::{album::*, group::*, library::*, ticket::*, user::*, *};

pub mod msg;
pub mod mysql;
pub mod svc;

// these are the database RPC calls that any backend server must be able to process
#[async_trait]
trait ESDbService: ESInner {
    // authdb functions

    // get this from checking all albums that contain the media + owning group of the library
    async fn media_access_groups(&self, media_uuid: MediaUuid) -> anyhow::Result<HashSet<String>>;

    async fn add_user(&self, user: User) -> anyhow::Result<()>;

    async fn get_user(&self, uid: String) -> anyhow::Result<User>;

    async fn delete_user(&self, uid: String) -> anyhow::Result<()>;

    async fn add_group(&self, group: Group) -> anyhow::Result<()>;

    async fn get_group(&self, gid: String) -> anyhow::Result<Group>;

    async fn delete_group(&self, gid: String) -> anyhow::Result<()>;

    async fn add_user_to_group(&self, uid: String, gid: String) -> anyhow::Result<()>;

    async fn rm_user_from_group(&self, uid: String, gid: String) -> anyhow::Result<()>;

    // media functions
    async fn add_media(&self, media: Media) -> anyhow::Result<MediaUuid>;

    async fn get_media(&self, media_uuid: MediaUuid) -> anyhow::Result<Media>;

    async fn get_media_by_path(&self, path: String) -> anyhow::Result<MediaUuid>;

    async fn update_media(
        &self,
        media_uuid: MediaUuid,
        change: MediaMetadata,
    ) -> anyhow::Result<()>;

    async fn set_media_hidden(
        &self,
        media_uuid: MediaUuid,
        hidden: bool,
    ) -> anyhow::Result<()>;

    async fn search_media(
        &self,
        user: String,
        filter: String,
    ) -> anyhow::Result<Vec<MediaUuid>>;

    // album functions
    async fn create_album(&self, album: Album) -> anyhow::Result<AlbumUuid>;

    async fn get_album(&self, album_uuid: AlbumUuid) -> anyhow::Result<Album>;

    async fn delete_album(&self, album_uuid: AlbumUuid) -> anyhow::Result<()>;

    async fn update_album(
        &self,
        album_uuid: AlbumUuid,
        change: AlbumMetadata,
    ) -> anyhow::Result<()>;

    async fn add_media_to_album(&self, media_uuid: MediaUuid, album_uuid: AlbumUuid) -> anyhow::Result<()>;

    async fn rm_media_from_album(&self, media_uuid: MediaUuid, album_uuid: AlbumUuid) -> anyhow::Result<()>;

    async fn search_albums(&self, user: String, filter: String) -> anyhow::Result<Vec<AlbumUuid>>;

    async fn search_media_in_album(
        &self,
        user: String,
        album_uuid: AlbumUuid,
        filter: String,
    ) -> anyhow::Result<Vec<MediaUuid>>;

    // library functions
    async fn add_library(&self, library: Library) -> anyhow::Result<LibraryUuid>;

    async fn get_library(&self, library_uuid: LibraryUuid) -> anyhow::Result<Library>;

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
