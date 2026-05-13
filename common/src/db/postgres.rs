use std::{
    collections::{HashMap, HashSet}, sync::Arc, time::{SystemTime, UNIX_EPOCH}
};

use anyhow::Result;
use async_trait::async_trait;
use bb8::Pool;
use bb8_postgres::PostgresConnectionManager;
use rustls::{ClientConfig, RootCertStore};
use rustls_native_certs::load_native_certs;
use serde::{Deserialize, Serialize};

use tokio_postgres_rustls::MakeRustlsConnect;
use tracing::{debug, info, instrument};
use url::Url;

use crate::{
    config::ESConfig,
    db::{DbBackend, MediaByCHash, MediaByPath},
};
use api::{
    UuidSource,
    collection::{Collection, CollectionUpdate, CollectionUuid},
    comment::{Comment, CommentUuid},
    library::{Library, LibraryUpdate, LibraryUuid},
    media::{Media, MediaUpdate, MediaUuid},
    search::SearchFilter,
};

fn set_to_hstore(set: HashSet<String>) -> HashMap<String, Option<String>> {
    set.into_iter().map(|s| (s, None)).collect()
}

fn hstore_to_set(hstore: HashMap<String, Option<String>>) -> HashSet<String> {
    hstore.into_keys().collect()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PostgresConfig {
    pub url: Url,
}

pub struct PostgresBackend {
    pool: Pool<PostgresConnectionManager<MakeRustlsConnect>>,
}

impl UuidSource for PostgresBackend {}

#[async_trait]
impl DbBackend for PostgresBackend {
    async fn new(config: Arc<ESConfig>) -> Result<Self> {
        info!("creating Postgres connection pool");

        let config = config
            .postgres
            .clone()
            .ok_or_else(|| anyhow::Error::msg("postgres config not present"))?;

        if config.url.scheme() != "postgres" {
            return Err(anyhow::Error::msg("invalid postgres url"))
        }

        let mut root_store = RootCertStore::empty();

        for cert in load_native_certs().certs {
            root_store.add(cert)?;
        }

        let tls_config = ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();

        let tls_config = MakeRustlsConnect::new(tls_config);

        let manager = PostgresConnectionManager::new_from_stringlike(config.url.as_str(), tls_config)?;

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

    #[instrument(skip(self))]
    async fn get_media(
        &self,
        media_uuid: MediaUuid,
    ) -> Result<Option<(Media, Vec<CollectionUuid>, Vec<CommentUuid>)>> {
        debug!("finding media details");
        let conn = self.pool.get_owned().await?;

        let media_statement = r#"-- get_media
            SELECT library_uuid, path, size, chash, phash, mtime, hidden, date, note, tags, media_type FROM media WHERE media_uuid = $1
        "#;

        let media_res = conn.query(media_statement, &[&media_uuid]).await?;

        let media_row = match media_res.first() {
            Some(ok) => ok,
            None => return Ok(None),
        };

        let media = Media {
            library_uuid: media_row.try_get("library_uuid")?,
            path: media_row.try_get("path")?,
            size: media_row.try_get::<&str, i64>("size")? as u64,
            chash: media_row.try_get("chash")?,
            phash: media_row.try_get("phash")?,
            mtime: media_row.try_get::<&str, i64>("mtime")? as u64,
            hidden: media_row.try_get("hidden")?,
            date: media_row.try_get("date")?,
            note: media_row.try_get("note")?,
            tags: hstore_to_set(media_row.try_get("tags")?),
            metadata: media_row.try_get("media_type")?,
        };

        let collection_statement = r#"-- get_media
            SELECT collection_uuid FROM collection_contents WHERE media_uuid = $1
        "#;

        let collections = conn
            .query_scalar(collection_statement, &[&media_uuid])
            .await?;

        let comment_statement = r#"-- get_media
            SELECT comment_uuid FROM comments WHERE media_uuid = $1
        "#;

        let comments = conn.query_scalar(comment_statement, &[&media_uuid]).await?;

        debug!("found media details");

        Ok(Some((media, collections, comments)))
    }

    #[instrument(skip(self))]
    async fn get_media_uuids(&self) -> Result<Vec<MediaUuid>> {
        debug!("finding all media uuids");

        let conn = self.pool.get().await?;

        let statement = r#"-- get_media_uuids
            SELECT media_uuid FROM media
        "#;

        let media_uuids = conn.query_scalar(statement, &[]).await?;

        debug!({ count = media_uuids.len() }, "found media");

        Ok(media_uuids)
    }

    #[instrument(skip(self))]
    async fn get_media_by_path(&self, path: String) -> Result<Option<MediaByPath>> {
        debug!("finding media by path");

        let conn = self.pool.get().await?;

        let statement = r#"-- get_media_by_path
            SELECT media_uuid, chash, mtime FROM media WHERE path = $1
        "#;

        let res = conn.query(statement, &[&path]).await?;

        let row = match res.first() {
            Some(ok) => ok,
            None => return Ok(None),
        };

        debug!("found media");

        Ok(Some(MediaByPath {
            media_uuid: row.try_get("media_uuid")?,
            hash: row.try_get("chash")?,
            mtime: row.try_get::<&str, i64>("mtime")? as u64,
        }))
    }

    #[instrument(skip(self))]
    async fn get_media_by_chash(
        &self,
        library_uuid: LibraryUuid,
        chash: String,
    ) -> Result<Option<MediaByCHash>> {
        debug!("finding media by content hash");

        let conn = self.pool.get().await?;

        let statement = r#"-- get_media_by_chash
            SELECT media_uuid, path, mtime FROM media WHERE library_uuid = $1 AND chash = $2
        "#;

        let res = conn.query(statement, &[&library_uuid, &chash]).await?;

        let row = match res.first() {
            Some(ok) => ok,
            None => return Ok(None),
        };

        debug!("found media");

        Ok(Some(MediaByCHash {
            media_uuid: row.try_get("media_uuid")?,
            path: row.try_get("path")?,
            mtime: row.try_get::<&str, i64>("mtime")? as u64,
        }))
    }

    #[instrument(skip(self, update))]
    async fn update_media(&self, media_uuid: MediaUuid, update: MediaUpdate) -> Result<()> {
        debug!("updating media details");

        let conn = self.pool.get().await?;

        let statement = r#"-- update_media
            UPDATE media SET
                hidden = COALESCE($1, hidden),
                date = COALESCE($2, date),
                note = COALESCE($3, note),
                tags = COALESCE($4, tags)
            WHERE media_uuid = $5
        "#;

        conn.query(
            statement,
            &[
                &update.hidden,
                &update.date,
                &update.note,
                &update.tags.map(set_to_hstore),
                &media_uuid,
            ],
        )
        .await?;

        debug!("updated media");

        Ok(())
    }

    #[instrument(skip(self))]
    async fn replace_media_path(
        &self,
        media_uuid: MediaUuid,
        path: String,
        hash: String,
        mtime: u64,
    ) -> Result<()> {
        debug!("replacing media path");

        let conn = self.pool.get().await?;

        let statement = r#"-- replace_media_path
            UPDATE media SET path = $1, chash = $2, mtime = $3 WHERE media_uuid = $4
        "#;

        conn.query_one(statement, &[&path, &hash, &(mtime as i64), &media_uuid])
            .await?;

        debug!("replaced media path");

        Ok(())
    }

    #[instrument(skip(self, filter))]
    async fn search_media(
        &self,
        gid: HashSet<String>,
        filter: SearchFilter,
    ) -> Result<Vec<MediaUuid>> {
        debug!("searching for media");

        let conn = self.pool.get().await?;

        let ts_search_sql = filter.format_postgres("media.ts_vec");

        // for a given uid and filter, find all media that match either:
        //  * is in a library owned by a group containing the uid
        //  * if the media is not hidden, is in an collection owned
        //    by a group containing the uid

        let mut statement = r#"-- search_media
            SELECT
                media.media_uuid
            FROM
                (
                    SELECT
                        media_uuid
                    FROM
                        (
                            SELECT
                                collection_uuid
                            FROM
                                collections
                            WHERE
                                gid = ANY($1)
                        ) AS t1
                        INNER JOIN collection_contents ON t1.collection_uuid = collection_contents.collection_uuid
                    UNION
                    SELECT
                        media_uuid
                    FROM
                        (
                            SELECT
                                library_uuid
                            FROM
                                libraries
                            WHERE
                                gid = ANY($1)
                        ) AS t2
                        INNER JOIN media ON t2.library_uuid = media.library_uuid
                ) AS t3
                INNER JOIN media ON t3.media_uuid = media.media_uuid
            WHERE
                media.hidden = FALSE"#.to_owned();

        statement.push_str(&ts_search_sql);

        let media = conn
            .query_scalar(&statement, &[&gid.into_iter().collect::<Vec<String>>()])
            .await?;

        debug!({ count = media.len() }, "found media");

        Ok(media)
    }

    #[instrument(skip(self))]
    async fn similar_media(
        &self,
        gid: HashSet<String>,
        media_uuid: MediaUuid,
        distance: i64,
    ) -> Result<Vec<MediaUuid>> {
        debug!("searching for similar media");

        let conn = self.pool.get().await?;

        let statement = r#"-- similar_media
            SELECT
                media.media_uuid
            FROM
                (
                    SELECT
                        media_uuid
                    FROM
                        (
                            SELECT
                                collection_uuid
                            FROM
                                collections
                            WHERE
                                gid = ANY($1)
                        ) AS t1
                        INNER JOIN collection_contents ON t1.collection_uuid = collection_contents.collection_uuid
                    UNION
                    SELECT
                        media_uuid
                    FROM
                        (
                            SELECT
                                library_uuid
                            FROM
                                libraries
                            WHERE
                                gid = ANY($1)
                        ) AS t2
                        INNER JOIN media ON t2.library_uuid = media.library_uuid
                ) AS t3
                INNER JOIN media ON t3.media_uuid = media.media_uuid
            WHERE
                media.hidden = FALSE
                AND bit_count(('x' || (SELECT phash FROM media WHERE media_uuid = $2))::bit & ('x' || media.phash)::bit) < $3
        "#;

        let media = conn
            .query_scalar(
                statement,
                &[
                    &gid.into_iter().collect::<Vec<String>>(),
                    &media_uuid,
                    &distance,
                ],
            )
            .await?;

        debug!({ count = media.len() }, "found similar media");

        Ok(media)
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
                    &(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64),
                    &comment.text,
                ],
            )
            .await?;

        debug!({ media_uuid = %comment.media_uuid, %comment_uuid }, "added comment");

        Ok(comment_uuid)
    }

    #[instrument(skip(self))]
    async fn get_comment(&self, comment_uuid: CommentUuid) -> Result<Option<Comment>> {
        debug!("finding comment");

        let conn = self.pool.get().await?;

        let statement = r#"-- get_comment
            SELECT media_uuid, uid, date, text FROM comments WHERE comment_uuid = $1
        "#;

        let res = conn.query(statement, &[&comment_uuid]).await?;

        let row = match res.first() {
            Some(ok) => ok,
            None => return Ok(None),
        };

        Ok(Some(Comment {
            media_uuid: row.try_get("media_uuid")?,
            uid: row.try_get("uid")?,
            date: row.try_get::<&str, i64>("date")? as u64,
            text: row.try_get("text")?,
        }))
    }

    #[instrument(skip(self))]
    async fn get_comment_uuids(&self) -> Result<Vec<CommentUuid>> {
        debug!("finding all comment uuids");

        let conn = self.pool.get().await?;

        let statement = r#"-- get_comment_uuids
            SELECT comment_uuid from comments
        "#;

        let comment_uuids = conn.query_scalar(statement, &[]).await?;

        debug!({ count = comment_uuids.len() }, "found comments");

        Ok(comment_uuids)
    }

    #[instrument(skip(self))]
    async fn delete_comment(&self, comment_uuid: CommentUuid) -> Result<()> {
        debug!("deleting comment");

        let conn = self.pool.get().await?;

        let statement = r#"-- delete_comment
            DELETE FROM comments WHERE comment_uuid = $1
        "#;

        conn.query(statement, &[&comment_uuid]).await?;

        Ok(())
    }

    #[instrument(skip(self, update))]
    async fn update_comment(
        &self,
        comment_uuid: CommentUuid,
        update: Option<String>,
    ) -> Result<()> {
        debug!("updating comment");

        let conn = self.pool.get().await?;

        let statement = r#"-- update_comment
            UPDATE comments SET
                text = COALESCE($1, text)
            WHERE comment_uuid = $2
        "#;

        conn.query(statement, &[&update, &comment_uuid]).await?;

        debug!("updated comment");

        Ok(())
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
            RETURNING collection_uuid
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

    #[instrument(skip(self))]
    async fn get_collection(&self, collection_uuid: CollectionUuid) -> Result<Option<Collection>> {
        debug!("finding collection");

        let conn = self.pool.get().await?;

        let statement = r#"-- get_collection
            SELECT uid, gid, name, note, tags, cover FROM collections WHERE collection_uuid = $1
        "#;

        let res = conn.query(statement, &[&collection_uuid]).await?;

        let row = match res.first() {
            Some(ok) => ok,
            None => return Ok(None),
        };

        debug!("found collection");

        Ok(Some(Collection {
            uid: row.try_get("uid")?,
            gid: row.try_get("gid")?,
            name: row.try_get("name")?,
            note: row.try_get("note")?,
            tags: hstore_to_set(row.try_get("tags")?),
            cover: row.try_get("cover")?,
        }))
    }

    #[instrument(skip(self))]
    async fn get_collection_uuids(&self) -> Result<Vec<CollectionUuid>> {
        debug!("finding all collection uuids");

        let conn = self.pool.get().await?;

        let statement = r#"-- get_collection_uuids
            SELECT collection_uuid from collections
        "#;

        let collection_uuids = conn.query_scalar(statement, &[]).await?;

        debug!({ count = collection_uuids.len() }, "found collections");

        Ok(collection_uuids)
    }

    #[instrument(skip(self))]
    async fn delete_collection(&self, collection_uuid: CollectionUuid) -> Result<()> {
        debug!("deleting collection");

        let conn = self.pool.get().await?;

        let statement = r#"-- delete_collection
            DELETE FROM collections WHERE collection_uuid = $1
        "#;

        conn.query(statement, &[&collection_uuid]).await?;

        Ok(())
    }

    #[instrument(skip(self, update))]
    async fn update_collection(
        &self,
        collection_uuid: CollectionUuid,
        update: CollectionUpdate,
    ) -> Result<()> {
        debug!("updating collection details");

        let conn = self.pool.get().await?;

        let statement = r#"-- update_collection
            UPDATE collections SET
                name = COALESCE($1, name),
                note = COALESCE($2, note),
                tags = COALESCE($3, tags)
            WHERE collection_uuid = $4
        "#;

        conn.query(
            statement,
            &[
                &update.name,
                &update.note,
                &update.tags.map(set_to_hstore),
                &collection_uuid,
            ],
        )
        .await?;

        debug!("updated collection");

        Ok(())
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

        let id: i64 = conn
            .query_one_scalar(statement, &[&media_uuid, &collection_uuid])
            .await?;

        debug!({ id }, "added media to collection");

        Ok(())
    }

    #[instrument(skip(self))]
    async fn rm_media_from_collection(
        &self,
        media_uuid: MediaUuid,
        collection_uuid: CollectionUuid,
    ) -> Result<()> {
        debug!("removing media from collection");

        let conn = self.pool.get().await?;

        let statement = r#"-- add_media_to_collection
            DELETE FROM collection_contents WHERE media_uuid = $1 AND collection_uuid = $2
        "#;

        conn.query_one(statement, &[&media_uuid, &collection_uuid])
            .await?;

        debug!("removed media from collection");

        Ok(())
    }

    #[instrument(skip(self, filter))]
    async fn search_collections(
        &self,
        gid: HashSet<String>,
        filter: SearchFilter,
    ) -> Result<Vec<CollectionUuid>> {
        debug!("searching for collections");

        let conn = self.pool.get().await?;

        let ts_search_sql = filter.format_postgres("collections.ts_vec");

        // for a given uid and filter, find all media that match either:
        //  * is in a library owned by a group containing the uid
        //  * if the media is not hidden, is in an collection owned
        //    by a group containing the uid

        let mut statement = r#"-- search_collections
            SELECT
                collection_uuid
            FROM
                collections
            WHERE
                gid = ANY($1)"#
            .to_owned();

        statement.push_str(&ts_search_sql);

        let collections = conn
            .query_scalar(&statement, &[&gid.into_iter().collect::<Vec<String>>()])
            .await?;

        debug!({ count = collections.len() }, "found collections");

        Ok(collections)
    }

    #[instrument(skip(self, filter))]
    async fn search_media_in_collection(
        &self,
        gid: HashSet<String>,
        collection_uuid: CollectionUuid,
        filter: SearchFilter,
    ) -> Result<Vec<MediaUuid>> {
        debug!("searching for media in collection");

        let conn = self.pool.get().await?;

        let ts_search_sql = filter.format_postgres("media.ts_vec");

        let mut statement = r#"-- search_media_in_collection
            SELECT
                media.media_uuid
            FROM
                (
                    SELECT
                        media_uuid
                    FROM
                        (
                            SELECT
                                collection_uuid
                            FROM
                                collections
                            WHERE
                                gid = ANY($1) AND collection_uuid = $2
                        ) AS t2
                        INNER JOIN collection_contents ON t2.collection_uuid = collection_contents.collection_uuid
                ) AS t3
                INNER JOIN media ON t3.media_uuid = media.media_uuid
            WHERE
                media.hidden = FALSE
        "#.to_owned();

        statement.push_str(&ts_search_sql);

        let media = conn
            .query_scalar(
                &statement,
                &[&gid.into_iter().collect::<Vec<String>>(), &collection_uuid],
            )
            .await?;

        debug!({ count = media.len() }, "found media in collection");

        Ok(media)
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

    #[instrument(skip(self))]
    async fn get_library(&self, library_uuid: LibraryUuid) -> Result<Option<Library>> {
        debug!("finding library");

        let conn = self.pool.get().await?;

        let statement = r#"-- get_library
            SELECT path, uid, gid, count FROM libraries WHERE library_uuid = $1
        "#;

        let res = conn.query(statement, &[&library_uuid]).await?;

        let row = match res.first() {
            Some(ok) => ok,
            None => return Ok(None),
        };

        debug!("found collection");

        Ok(Some(Library {
            path: row.try_get("path")?,
            uid: row.try_get("uid")?,
            gid: row.try_get("gid")?,
            count: row.try_get("count")?,
        }))
    }

    #[instrument(skip(self))]
    async fn get_library_uuids(&self) -> Result<Vec<LibraryUuid>> {
        debug!("finding all library uuids");

        let conn = self.pool.get().await?;

        let statement = r#"-- get_library_uuids
            SELECT library_uuid from libraries
        "#;

        let library_uuids = conn.query_scalar(statement, &[]).await?;

        debug!({ count = library_uuids.len() }, "found libraries");

        Ok(library_uuids)
    }

    #[instrument(skip(self, update))]
    async fn update_library(&self, library_uuid: LibraryUuid, update: LibraryUpdate) -> Result<()> {
        debug!("updating library details");

        let conn = self.pool.get().await?;

        let statement = r#"-- update_library
            UPDATE libraries SET
                count = COALESCE($1, count)
            WHERE library_uuid = $2
        "#;

        conn.query(statement, &[&update.count, &library_uuid])
            .await?;

        debug!("updated library");

        Ok(())
    }

    async fn search_libraries(
        &self,
        gid: HashSet<String>,
        filter: String,
    ) -> Result<Vec<LibraryUuid>> {
        debug!("searching for libraries");

        let conn = self.pool.get().await?;

        let statement = r#"-- search_libraries
            SELECT
                library_uuid
            FROM
                libraries
            WHERE
                gid = ANY($1) AND path LIKE $2
        "#;

        let libraries = conn
            .query_scalar(
                statement,
                &[
                    &gid.into_iter().collect::<Vec<String>>(),
                    &format!("%{}%", filter),
                ],
            )
            .await?;

        debug!({ count = libraries.len() }, "found libraries");

        Ok(libraries)
    }

    #[instrument(skip(self))]
    async fn search_media_in_library(
        &self,
        gid: HashSet<String>,
        library_uuid: LibraryUuid,
        hidden: Option<bool>,
        filter: SearchFilter,
    ) -> Result<Vec<MediaUuid>> {
        debug!("searching for media in library");

        let conn = self.pool.get().await?;

        let ts_search_sql = filter.format_postgres("media.ts_vec");

        let mut statement = r#"-- search_media_in_collection
            SELECT
                media.media_uuid
            FROM
                (
                    SELECT
                        library_uuid
                    FROM
                        libraries
                    WHERE
                        gid = ANY($1) AND library_uuid = $2
                ) AS t1
                INNER JOIN media ON t1.library_uuid = media.library_uuid
            WHERE
                media.hidden = COALESCE($3, media.hidden)"#.to_owned();

        statement.push_str(&ts_search_sql);

        let media = conn
            .query_scalar(
                &statement,
                &[&gid.into_iter().collect::<Vec<String>>(), &library_uuid, &hidden],
            )
            .await?;

        debug!({ count = media.len() }, "found media in collection");

        Ok(media)
    }
}
