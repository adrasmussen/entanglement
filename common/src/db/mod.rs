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
    async fn media_access_groups(&self, media_uuid: MediaUuid) -> anyhow::Result<HashSet<String>>;

    // media functions
    async fn add_media(&self, media: Media) -> anyhow::Result<MediaUuid>;

    async fn get_media(
        &self,
        media_uuid: MediaUuid,
    ) -> anyhow::Result<Option<(Media, Vec<CollectionUuid>, Vec<CommentUuid>)>>;

    async fn get_media_uuid_by_path(&self, path: String) -> anyhow::Result<Option<MediaUuid>>;

    async fn update_media(&self, media_uuid: MediaUuid, update: MediaUpdate) -> anyhow::Result<()>;

    async fn search_media(
        &self,
        uid: String,
        gid: HashSet<String>,
        filter: String,
    ) -> anyhow::Result<Vec<MediaUuid>>;

    async fn similar_media(
        &self,
        uid: String,
        gid: HashSet<String>,
        media_uuid: MediaUuid,
        distance: i64,
    ) -> anyhow::Result<Vec<MediaUuid>>;

    // comment functions
    async fn add_comment(&self, comment: Comment) -> anyhow::Result<CommentUuid>;

    async fn get_comment(&self, comment_uuid: CommentUuid) -> anyhow::Result<Option<Comment>>;

    async fn delete_comment(&self, comment_uuid: CommentUuid) -> anyhow::Result<()>;

    async fn update_comment(
        &self,
        comment_uuid: CommentUuid,
        text: Option<String>,
    ) -> anyhow::Result<()>;

    // collection functions
    async fn add_collection(&self, collection: Collection) -> anyhow::Result<CollectionUuid>;

    async fn get_collection(
        &self,
        collection_uuid: CollectionUuid,
    ) -> anyhow::Result<Option<Collection>>;

    async fn delete_collection(&self, collection_uuid: CollectionUuid) -> anyhow::Result<()>;

    async fn update_collection(
        &self,
        collection_uuid: CollectionUuid,
        update: CollectionUpdate,
    ) -> anyhow::Result<()>;

    async fn add_media_to_collection(
        &self,
        media_uuid: MediaUuid,
        collection_uuid: CollectionUuid,
    ) -> anyhow::Result<()>;

    async fn rm_media_from_collection(
        &self,
        media_uuid: MediaUuid,
        collection_uuid: CollectionUuid,
    ) -> anyhow::Result<()>;

    async fn search_collections(
        &self,
        uid: String,
        gid: HashSet<String>,
        filter: String,
    ) -> anyhow::Result<Vec<CollectionUuid>>;

    async fn search_media_in_collection(
        &self,
        uid: String,
        gid: HashSet<String>,
        collection_uuid: CollectionUuid,
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

    async fn search_libraries(
        &self,
        uid: String,
        gid: HashSet<String>,
        filter: String,
    ) -> anyhow::Result<Vec<LibraryUuid>>;

    async fn search_media_in_library(
        &self,
        uid: String,
        gid: HashSet<String>,
        uuid: LibraryUuid,
        filter: String,
        hidden: bool,
    ) -> anyhow::Result<Vec<MediaUuid>>;
}
