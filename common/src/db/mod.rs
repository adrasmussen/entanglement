use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;

use crate::config::ESConfig;
use api::{
    collection::{Collection, CollectionUpdate, CollectionUuid},
    comment::{Comment, CommentUuid},
    library::{Library, LibraryUpdate, LibraryUuid},
    media::{Media, MediaUpdate, MediaUuid},
    search::SearchFilter,
};

pub mod mariadb;
pub use mariadb::MariaDBBackend;

// these are the database RPC calls that any backend server must be able to process
#[async_trait]
pub trait DbBackend: Send + Sync + 'static {
    fn new(config: Arc<ESConfig>) -> Result<Self>
    where
        Self: Sized;

    // get this from checking all collections that contain the media + owning group of the library
    async fn media_access_groups(&self, media_uuid: MediaUuid) -> Result<HashSet<String>>;

    // media functions
    async fn add_media(&self, media: Media) -> Result<MediaUuid>;

    async fn get_media(
        &self,
        media_uuid: MediaUuid,
    ) -> Result<Option<(Media, Vec<CollectionUuid>, Vec<CommentUuid>)>>;

    async fn get_media_uuid_by_path(&self, path: String) -> Result<Option<MediaUuid>>;

    async fn get_media_uuid_by_chash(&self, chash: String) -> Result<Option<MediaUuid>>;

    async fn update_media(&self, media_uuid: MediaUuid, update: MediaUpdate) -> Result<()>;

    async fn replace_media_path(&self, media_uuid: MediaUuid, path: String) -> Result<()>;

    async fn search_media(
        &self,
        gid: HashSet<String>,
        filter: SearchFilter,
    ) -> Result<Vec<MediaUuid>>;

    async fn similar_media(
        &self,
        gid: HashSet<String>,
        media_uuid: MediaUuid,
        distance: i64,
    ) -> Result<Vec<MediaUuid>>;

    // comment functions
    async fn add_comment(&self, comment: Comment) -> Result<CommentUuid>;

    async fn get_comment(&self, comment_uuid: CommentUuid) -> Result<Option<Comment>>;

    async fn delete_comment(&self, comment_uuid: CommentUuid) -> Result<()>;

    async fn update_comment(&self, comment_uuid: CommentUuid, text: Option<String>) -> Result<()>;

    // collection functions
    async fn add_collection(&self, collection: Collection) -> Result<CollectionUuid>;

    async fn get_collection(&self, collection_uuid: CollectionUuid) -> Result<Option<Collection>>;

    async fn delete_collection(&self, collection_uuid: CollectionUuid) -> Result<()>;

    async fn update_collection(
        &self,
        collection_uuid: CollectionUuid,
        update: CollectionUpdate,
    ) -> Result<()>;

    async fn add_media_to_collection(
        &self,
        media_uuid: MediaUuid,
        collection_uuid: CollectionUuid,
    ) -> Result<()>;

    async fn rm_media_from_collection(
        &self,
        media_uuid: MediaUuid,
        collection_uuid: CollectionUuid,
    ) -> Result<()>;

    async fn search_collections(
        &self,
        gid: HashSet<String>,
        filter: SearchFilter,
    ) -> Result<Vec<CollectionUuid>>;

    async fn search_media_in_collection(
        &self,
        gid: HashSet<String>,
        collection_uuid: CollectionUuid,
        filter: SearchFilter,
    ) -> Result<Vec<MediaUuid>>;

    // library functions
    async fn add_library(&self, library: Library) -> Result<LibraryUuid>;

    async fn get_library(&self, library_uuid: LibraryUuid) -> Result<Option<Library>>;

    async fn update_library(&self, library_uuid: LibraryUuid, update: LibraryUpdate) -> Result<()>;

    async fn search_libraries(
        &self,
        gid: HashSet<String>,
        filter: String,
    ) -> Result<Vec<LibraryUuid>>;

    async fn search_media_in_library(
        &self,
        gid: HashSet<String>,
        uuid: LibraryUuid,
        hidden: bool,
        filter: SearchFilter,
    ) -> Result<Vec<MediaUuid>>;
}
