use std::collections::HashSet;

use anyhow;

use async_trait::async_trait;

use crate::service::ESInner;
use common::api::{album::*, comment::*, library::*, media::*};

pub mod msg;
pub mod mysql;
pub mod svc;

// these are the database RPC calls that any backend server must be able to process
#[async_trait]
trait ESDbService: ESInner {
    // authdb functions

    // get this from checking all albums that contain the media + owning group of the library
    async fn media_access_groups(&self, media_uuid: MediaUuid) -> anyhow::Result<HashSet<String>>;

    // media functions
    async fn add_media(&self, media: Media) -> anyhow::Result<MediaUuid>;

    async fn get_media(&self, media_uuid: MediaUuid) -> anyhow::Result<Option<(Media, Vec<AlbumUuid>, Vec<CommentUuid>)>>;

    async fn get_media_uuid_by_path(&self, path: String) -> anyhow::Result<Option<MediaUuid>>;

    async fn update_media(
        &self,
        media_uuid: MediaUuid,
        update: MediaUpdate,
    ) -> anyhow::Result<()>;

    async fn search_media(&self, uid: String, filter: String) -> anyhow::Result<Vec<MediaUuid>>;

    // album functions
    async fn add_album(&self, album: Album) -> anyhow::Result<AlbumUuid>;

    async fn get_album(&self, album_uuid: AlbumUuid) -> anyhow::Result<Option<Album>>;

    async fn delete_album(&self, album_uuid: AlbumUuid) -> anyhow::Result<()>;

    async fn update_album(
        &self,
        album_uuid: AlbumUuid,
        update: AlbumUpdate,
    ) -> anyhow::Result<()>;

    async fn add_media_to_album(
        &self,
        media_uuid: MediaUuid,
        album_uuid: AlbumUuid,
    ) -> anyhow::Result<()>;

    async fn rm_media_from_album(
        &self,
        media_uuid: MediaUuid,
        album_uuid: AlbumUuid,
    ) -> anyhow::Result<()>;

    async fn search_albums(&self, uid: String, filter: String) -> anyhow::Result<Vec<AlbumUuid>>;

    async fn search_media_in_album(
        &self,
        uid: String,
        album_uuid: AlbumUuid,
        filter: String,
    ) -> anyhow::Result<Vec<MediaUuid>>;

    // library functions
    async fn add_library(&self, library: Library) -> anyhow::Result<LibraryUuid>;

    async fn get_library(&self, library_uuid: LibraryUuid) -> anyhow::Result<Option<Library>>;

    async fn update_library(
        &self,
        library_uuid: LibraryUuid,
        update: LibraryUpdate,
    ) -> anyhow::Result<()>;

    async fn search_libraries(&self, uid: String, filter: String) -> anyhow::Result<Vec<LibraryUuid>>;

    async fn search_media_in_library(
        &self,
        uid: String,
        uuid: LibraryUuid,
        filter: String,
        hidden: bool,
    ) -> anyhow::Result<Vec<MediaUuid>>;
}
