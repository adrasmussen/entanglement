use std::{
    collections::HashSet,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use async_trait::async_trait;
use bb8::Pool;
use bb8_postgres::PostgresConnectionManager;
use postgres::NoTls;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, instrument};

use crate::{
    config::ESConfig,
    db::{DbBackend, MediaByCHash, MediaByPath},
};
use api::{
    collection::{Collection, CollectionUpdate, CollectionUuid},
    comment::{Comment, CommentUuid},
    fold_set,
    library::{Library, LibraryUpdate, LibraryUuid},
    media::{Media, MediaMetadata, MediaUpdate, MediaUuid},
    search::SearchFilter,
    unfold_set,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PostgresConfig {
    pub url: String,
}

pub struct PostgresBackend {
    pool: Pool<PostgresConnectionManager<NoTls>>,
}

#[async_trait]
impl DbBackend for PostgresBackend {
    async fn new(config: Arc<ESConfig>) -> Result<Self> {
        info!("creating Postgres connection pool");

        let url = config
            .postgres
            .clone()
            .expect("postgres config not present")
            .url;

        // TODO -- YesTls
        let manager = PostgresConnectionManager::new_from_stringlike(url, NoTls {})?;

        let pool = Pool::builder().build(manager).await?;

        Ok(Self { pool })
    }

    #[instrument(skip(self))]
    async fn media_access_groups(&self, media_uuid: MediaUuid) -> Result<HashSet<String>> {
        debug!("finding media access groups");

        let conn = self.pool.get().await?;

        let statement = r"-- media_access_groups
        SELECT
            gid
        FROM
            collections
        INNER JOIN collection_contents ON collections.collection_uuid = collection_contents.collection_uuid
        INNER JOIN media ON collection_contents.media_uuid = media.media_uuid
        WHERE
            media.media_uuid = $1 AND media.hidden = FALSE
        UNION
        SELECT
            gid
        FROM
            libraries
        INNER JOIN media ON libraries.library_uuid = media.library_uuid
        WHERE
            media.media_uuid = $1
        ";

        let data = conn
            .query_scalar::<String, str>(statement, &[&media_uuid.to_string()])
            .await?
            .into_iter()
            .collect();

        debug!({ groups = ?data }, "found groups");

        Ok(data)
    }

    // media queries
    #[instrument(skip(self, media))]
    async fn add_media(&self, media: Media) -> Result<MediaUuid> {
        debug!({ media_path = media.path }, "adding media");

        let conn = self.pool.get().await?;

        let statement = r"-- add_media
            INSERT INTO media (media_uuid, library_uuid, path, size, chash, phash, mtime, hidden, date, note, tags, media_type)
            VALUES (UUID7(), $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (library_uuid, path) DO NOTHING
            RETURNING media_uuid
        ";

        //let data = conn.query_one_scalar(statement, &[&(media.library_uuid as i64), &media.path, &media.size, &media.chash, &media.phash, &media.mtime, &media.hidden, &media.date, &media.note, &media.tags, &media.metadata]).await?;

        todo!()
    }

    async fn get_media(
        &self,
        media_uuid: MediaUuid,
    ) -> Result<Option<(Media, Vec<CollectionUuid>, Vec<CommentUuid>)>> {
        todo!()
    }

    async fn get_media_uuids(&self) -> Result<Vec<MediaUuid>> {
        todo!()
    }

    async fn get_media_by_path(&self, path: String) -> Result<Option<MediaByPath>> {
        todo!()
    }

    async fn get_media_by_chash(
        &self,
        library_uuid: LibraryUuid,
        chash: String,
    ) -> Result<Option<MediaByCHash>> {
        todo!()
    }

    async fn update_media(&self, media_uuid: MediaUuid, update: MediaUpdate) -> Result<()> {
        todo!()
    }

    async fn replace_media_path(
        &self,
        media_uuid: MediaUuid,
        path: String,
        hash: String,
        mtime: u64,
    ) -> Result<()> {
        todo!()
    }

    async fn search_media(
        &self,
        gid: HashSet<String>,
        filter: SearchFilter,
    ) -> Result<Vec<MediaUuid>> {
        todo!()
    }

    async fn similar_media(
        &self,
        gid: HashSet<String>,
        media_uuid: MediaUuid,
        distance: i64,
    ) -> Result<Vec<MediaUuid>> {
        todo!()
    }

    // comment functions
    async fn add_comment(&self, comment: Comment) -> Result<CommentUuid> {
        todo!()
    }

    async fn get_comment(&self, comment_uuid: CommentUuid) -> Result<Option<Comment>> {
        todo!()
    }

    async fn get_comment_uuids(&self) -> Result<Vec<CommentUuid>> {
        todo!()
    }

    async fn delete_comment(&self, comment_uuid: CommentUuid) -> Result<()> {
        todo!()
    }

    async fn update_comment(&self, comment_uuid: CommentUuid, text: Option<String>) -> Result<()> {
        todo!()
    }

    // collection functions
    async fn add_collection(&self, collection: Collection) -> Result<CollectionUuid> {
        todo!()
    }

    async fn get_collection(&self, collection_uuid: CollectionUuid) -> Result<Option<Collection>> {
        todo!()
    }

    async fn get_collection_uuids(&self) -> Result<Vec<CollectionUuid>> {
        todo!()
    }

    async fn delete_collection(&self, collection_uuid: CollectionUuid) -> Result<()> {
        todo!()
    }

    async fn update_collection(
        &self,
        collection_uuid: CollectionUuid,
        update: CollectionUpdate,
    ) -> Result<()> {
        todo!()
    }

    async fn add_media_to_collection(
        &self,
        media_uuid: MediaUuid,
        collection_uuid: CollectionUuid,
    ) -> Result<()> {
        todo!()
    }

    async fn rm_media_from_collection(
        &self,
        media_uuid: MediaUuid,
        collection_uuid: CollectionUuid,
    ) -> Result<()> {
        todo!()
    }

    async fn search_collections(
        &self,
        gid: HashSet<String>,
        filter: SearchFilter,
    ) -> Result<Vec<CollectionUuid>> {
        todo!()
    }

    async fn search_media_in_collection(
        &self,
        gid: HashSet<String>,
        collection_uuid: CollectionUuid,
        filter: SearchFilter,
    ) -> Result<Vec<MediaUuid>> {
        todo!()
    }

    // library functions
    async fn add_library(&self, library: Library) -> Result<LibraryUuid> {
        todo!()
    }

    async fn get_library(&self, library_uuid: LibraryUuid) -> Result<Option<Library>> {
        todo!()
    }

    async fn get_library_uuids(&self) -> Result<Vec<LibraryUuid>> {
        todo!()
    }

    async fn update_library(&self, library_uuid: LibraryUuid, update: LibraryUpdate) -> Result<()> {
        todo!()
    }

    async fn search_libraries(
        &self,
        gid: HashSet<String>,
        filter: String,
    ) -> Result<Vec<LibraryUuid>> {
        todo!()
    }

    async fn search_media_in_library(
        &self,
        gid: HashSet<String>,
        uuid: LibraryUuid,
        hidden: Option<bool>,
        filter: SearchFilter,
    ) -> Result<Vec<MediaUuid>> {
        todo!()
    }
}
