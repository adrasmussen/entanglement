use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use async_trait::async_trait;
use bb8::Pool;
use bb8_postgres::PostgresConnectionManager;
use rustls::{ClientConfig, RootCertStore};
use rustls_native_certs::load_native_certs;
use serde::{Deserialize, Serialize};
use tokio_postgres_rustls::MakeRustlsConnect;
use tracing::{debug, error, info, instrument};
use uuid::Uuid;

use crate::{
    config::ESConfig,
    db::{DbBackend, MediaByCHash, MediaByPath},
};
use api::{
    UuidSource,
    collection::{Collection, CollectionUpdate, CollectionUuid},
    comment::{Comment, CommentUuid},
    library::{Library, LibraryUpdate, LibraryUuid},
    media::{Media, MediaMetadata, MediaUpdate, MediaUuid},
    search::SearchFilter,
};

fn set_to_hstore(set: HashSet<String>) -> HashMap<String, Option<String>> {
    set.into_iter().map(|s| (s, None)).collect()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PostgresConfig {
    pub url: String,
}

pub struct PostgresBackend {
    pool: Pool<PostgresConnectionManager<MakeRustlsConnect>>,
}

impl UuidSource for PostgresBackend {}

#[async_trait]
impl DbBackend for PostgresBackend {
    async fn new(config: Arc<ESConfig>) -> Result<Self> {
        info!("creating Postgres connection pool");

        let url = config
            .postgres
            .clone()
            .expect("postgres config not present")
            .url;

        let mut root_store = RootCertStore::empty();

        for cert in load_native_certs().certs {
            root_store.add(cert)?;
        }

        let tls_config = ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();

        let tls = MakeRustlsConnect::new(tls_config);

        let manager = PostgresConnectionManager::new_from_stringlike(url, tls)?;

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
            .query_scalar::<String, str>(statement, &[&media_uuid])
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
            VALUES (uuidv7(), $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (library_uuid, path) DO NOTHING
            RETURNING media_uuid
        ";

        let media_uuid: MediaUuid = conn
            .query_one_scalar(
                statement,
                &[
                    &media.library_uuid,
                    &media.path,
                    &(media.size as i64),
                    &media.chash,
                    &media.phash,
                    &(media.mtime as i64),
                    &media.hidden,
                    &media.date,
                    &media.note,
                    &set_to_hstore(media.tags),
                    &media.metadata,
                ],
            )
            .await?;

        debug!({ media_path = media.path, %media_uuid }, "added media");

        Ok(media_uuid)
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
    #[instrument(skip(self, comment))]
    async fn add_comment(&self, comment: Comment) -> Result<CommentUuid> {
        debug!({ media_uuid = %comment.media_uuid }, "adding comment");

        let conn = self.pool.get().await?;

        let statement = r#"-- add_comment
            INSERT INTO comments (comment_uuid, media_uuid, uid, date, text)
            VALUES (uuidv7(), $1, $2, $3, $4)
            RETURNING comment_uuid
        "#;

        let comment_uuid: CommentUuid = conn
            .query_one_scalar(
                statement,
                &[
                    &comment.media_uuid,
                    &comment.uid,
                    &(comment.date as i64),
                    &comment.text,
                ],
            )
            .await?;

            debug!({ media_uuid = %comment.media_uuid, %comment_uuid }, "added comment");

        Ok(comment_uuid)
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
    #[instrument(skip(self, collection))]
    async fn add_collection(&self, collection: Collection) -> Result<CollectionUuid> {
        debug!({ collection_name = collection.name }, "adding collection");

        let conn = self.pool.get().await?;

        let statement = r"-- add_collection
            INSERT INTO collections (collection_uuid, uid, gid, name, note, tags, cover)
            VALUES (uuidv7(), $1, $2, $3, $4, $5, $6)
            ON CONFLICT (uid, name) DO NOTHING
            RETURNING media_uuid
        ";

        let collection_uuid: CollectionUuid = conn
            .query_one_scalar(
                statement,
                &[
                    &collection.uid,
                    &collection.gid,
                    &collection.name,
                    &collection.note,
                    &set_to_hstore(collection.tags),
                    &collection.cover,
                ],
            )
            .await?;

        debug!({ collection_name = collection.name, %collection_uuid }, "added collection");

        Ok(collection_uuid)
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

    #[instrument(skip(self))]
    async fn add_media_to_collection(
        &self,
        media_uuid: MediaUuid,
        collection_uuid: CollectionUuid,
    ) -> Result<()> {
        debug!("adding media to collection");

        let conn = self.pool.get().await?;

        let statement = r#"-- add_media_to_collection
            INSERT INTO collection_contents (media_uuid, collection_uuid)
            VALUES ($1, $2)
            ON CONFLICT (media_uuid, collection_uuid) DO NOTHING
            RETURNING id

        "#;

        let data: i64 = conn
            .query_one_scalar(
                statement,
                &[&media_uuid, &collection_uuid],
            )
            .await?;

        debug!({ id = data }, "added media to collection");

        Ok(())
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
    #[instrument(skip(self, library))]
    async fn add_library(&self, library: Library) -> Result<LibraryUuid> {
        debug!({ library_path = library.path }, "adding library");

        let conn = self.pool.get().await?;

        let statement = r"-- add_library
            INSERT INTO libraries (library_uuid, path, gid, count)
            VALUES (uuidv7(), $1, $2, $3)
            ON CONFLICT (path) DO NOTHING
            RETURNING library_uuid
        ";

        let library_uuid: LibraryUuid = conn
            .query_one_scalar(statement, &[&library.path, &library.gid, &library.count])
            .await?;

        debug!({ library_path = library.path , %library_uuid }, "added library");

        Ok(library_uuid)
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
