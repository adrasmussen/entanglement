use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, LazyLock},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use async_trait::async_trait;
use mysql_async::{FromRowError, Params, Pool, Row, Value, from_row_opt, prelude::*};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument};
use url::Url;
use uuid::Uuid;

use crate::{
    config::ESConfig,
    db::{DbBackend, MediaByCHash, MediaByPath, MediaDetails, PageOptions, SearchPage},
};
use api::{
    FOLDING_SEPARATOR, UuidSource,
    collection::{Collection, CollectionUpdate, CollectionUuid},
    comment::{Comment, CommentUuid},
    fold_set,
    library::{Library, LibraryUpdate, LibraryUuid},
    media::{Media, MediaMetadata, MediaUpdate, MediaUuid},
    search::{SearchOptions, escape_like},
    unfold_set,
};

// mariadb backend
//
// it is almost certainly the case that this backend is not laid out correctly,
// which is almost trivially true because we are using set folding instead of
// a tag table with a pivot
//
// worse, because of the overlapping indices, we have to manually RwLock the
// tables to prevent massive collision pileup when adding media to the system.
//
// on the flip side, the queries are relatively straightforward to reason about
// and have an easy realization in terms of the api::search::SearchFilter tools.

// missing calls to-do
//
// delete_media
// delete_library

// sanity check to-do
//
// calls that should be transactions for consistency:
//  * add_media
//  * delete_collection
//
// update_* can fail partway through
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MariaDbConfig {
    pub url: Url,
}

pub struct MariaDBBackend {
    pool: Pool,
    locks: TableLocks,
}

#[derive(Default)]
struct TableLocks {
    media: RwLock<()>,
    comment: RwLock<()>,
    library: RwLock<()>,
    contents: RwLock<()>,
    collection: RwLock<()>,
}

impl UuidSource for MariaDBBackend {}

#[async_trait]
impl DbBackend for MariaDBBackend {
    async fn new(config: Arc<ESConfig>) -> Result<Self> {
        info!("creating MariaDB connection pool");

        let config = config
            .mariadb
            .clone()
            .ok_or_else(|| anyhow::Error::msg("mariadb config not present"))?;

        if config.url.scheme() != "mysql" {
            return Err(anyhow::Error::msg("invalid mariadb url"));
        }

        Ok(Self {
            pool: Pool::new(config.url.as_str()),
            locks: TableLocks::default(),
        })
    }

    #[instrument(skip(self))]
    async fn media_access_groups(&self, media_uuid: MediaUuid) -> Result<HashSet<String>> {
        debug!("finding media access groups");

        let _mr = self.locks.media.read().await;
        let _lr = self.locks.library.read().await;
        let _xr = self.locks.contents.read().await;
        let _cr = self.locks.collection.read().await;

        // for a given media_uuid, find all gids that match either:
        //  * if the media is not hidden, any collection that contains the media
        //  * the library that contains that media
        let result = r"
            SELECT
                gid
            FROM
                collections
            INNER JOIN collection_contents ON collections.collection_uuid = collection_contents.collection_uuid
            INNER JOIN media ON collection_contents.media_uuid = media.media_uuid
            WHERE
                media.media_uuid = :media_uuid AND media.hidden = FALSE
            UNION
            SELECT
                gid
            FROM
                libraries
            INNER JOIN media ON libraries.library_uuid = media.library_uuid
            WHERE
                media.media_uuid = :media_uuid"
            .with(params! {
                "media_uuid" => media_uuid.value(),
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let data = result
            .into_iter()
            .map(from_row_opt::<String>)
            .collect::<Result<HashSet<_>, FromRowError>>()?;

        debug!({ groups = ?data }, "found groups");

        Ok(data)
    }

    // media queries
    #[instrument(skip(self, media))]
    async fn add_media(&self, media: Media) -> Result<MediaUuid> {
        debug!({ media_path = media.path }, "adding media");

        let _mw = self.locks.media.write().await;
        let _lw = self.locks.library.write().await;

        let query = r"
            INSERT INTO media (media_uuid, library_uuid, path, size, chash, phash, mtime, hidden, date, note, tags, media_type)
            SELECT
                UUID_v7(),
                :library_uuid,
                :path,
                :size,
                :chash,
                :phash,
                :mtime,
                :hidden,
                :date,
                :note,
                :tags,
                :media_type
            FROM
                DUAL
            WHERE NOT EXISTS(
                SELECT 1
                FROM media
                WHERE
                    library_uuid = :library_uuid
                    AND path = :path
            )
            RETURNING media_uuid";

        let mut result = query
            .with(params! {
                "library_uuid" => media.library_uuid.value(),
                "path" => media.path.clone(),
                "size" => media.size,
                "chash" => media.chash,
                "phash" => media.phash,
                "mtime" => media.mtime,
                "hidden" => media.hidden,
                "date" => media.date,
                "note" => media.note,
                "tags" => fold_set(media.tags)?,
                "media_type" => match media.metadata {
                    MediaMetadata::Image => "Image",
                    MediaMetadata::Video => "Video",
                    MediaMetadata::VideoSlice => "VideoSlice",
                    MediaMetadata::Audio => "Audio"
                },
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let row = result.pop().ok_or_else(|| {
            error!({ media_path = media.path }, "failed to add media");
            anyhow::Error::msg("failed to add media")
        })?;

        let media_uuid = from_row_opt::<Uuid>(row)?;

        debug!({ media_path = media.path, %media_uuid }, "added media");

        Ok(MediaUuid::from_value(self, media_uuid))
    }

    #[instrument(skip(self))]
    async fn get_media(
        &self,
        media_uuid: MediaUuid,
    ) -> Result<Option<(Media, Vec<CollectionUuid>, Vec<CommentUuid>)>> {
        debug!("getting media details");

        let _mr = self.locks.media.read().await;
        let _yr = self.locks.comment.read().await;
        let _xr = self.locks.contents.read().await;

        let mut media_result =
            format!("SELECT {MEDIA_RECORD_COLS} FROM media WHERE media_uuid = :media_uuid")
                .with(params! {
                    "media_uuid" => media_uuid.value(),
                })
                .run(self.pool.get_conn().await?)
                .await?
                .collect::<Row>()
                .await?;

        let media = match media_result.pop() {
            Some(row) => media_from_row(self, row)?.1,
            None => return Ok(None),
        };

        let collection_result = r"
            SELECT collection_uuid FROM collection_contents WHERE media_uuid = :media_uuid"
            .with(params! {
                "media_uuid" => media_uuid.value(),
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let collection_data = collection_result
            .into_iter()
            .map(|row| {
                let input = from_row_opt::<Uuid>(row)?;

                Ok(CollectionUuid::from_value(self, input))
            })
            .collect::<Result<Vec<CollectionUuid>, FromRowError>>()?;

        let comment_result = r"
            SELECT comment_uuid FROM comments WHERE media_uuid = :media_uuid"
            .with(params! {
                "media_uuid" => media_uuid.value(),
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let comment_data = comment_result
            .into_iter()
            .map(|row| {
                let input = from_row_opt::<Uuid>(row)?;

                Ok(CommentUuid::from_value(self, input))
            })
            .collect::<Result<Vec<CommentUuid>, FromRowError>>()?;

        debug!("found media details");

        Ok(Some((media, collection_data, comment_data)))
    }

    #[instrument(skip(self, media_uuids))]
    async fn get_media_batch(&self, media_uuids: Vec<MediaUuid>) -> Result<Vec<MediaDetails>> {
        debug!(
            { count = media_uuids.len() },
            "getting media details in batch"
        );

        if media_uuids.is_empty() {
            return Ok(Vec::new());
        }

        let _mr = self.locks.media.read().await;
        let _yr = self.locks.comment.read().await;
        let _xr = self.locks.contents.read().await;

        // the IN list is only ever "?, ?, ..." -- the uuids themselves are bound positionally
        let placeholders = vec!["?"; media_uuids.len()].join(", ");
        let values = media_uuids.iter().map(|m| m.value()).collect::<Vec<Uuid>>();

        let media_rows =
            format!("SELECT {MEDIA_RECORD_COLS} FROM media WHERE media_uuid IN ({placeholders})")
                .with(values.clone())
                .run(self.pool.get_conn().await?)
                .await?
                .collect::<Row>()
                .await?;

        let mut media = HashMap::with_capacity(media_rows.len());
        for row in media_rows {
            let (media_uuid, m) = media_from_row(self, row)?;
            media.insert(media_uuid, m);
        }

        let mut collections = self
            .group_by_media(
                format!("SELECT media_uuid, collection_uuid FROM collection_contents WHERE media_uuid IN ({placeholders})"),
                values.clone(),
                |c| CollectionUuid::from_value(self, c),
            )
            .await?;

        let mut comments = self
            .group_by_media(
                format!("SELECT media_uuid, comment_uuid FROM comments WHERE media_uuid IN ({placeholders})"),
                values,
                |c| CommentUuid::from_value(self, c),
            )
            .await?;

        let out = media_uuids
            .into_iter()
            .filter_map(|media_uuid| {
                let key = media_uuid.value();
                media.remove(&key).map(|m| MediaDetails {
                    media_uuid,
                    media: m,
                    collections: collections.remove(&key).unwrap_or_default(),
                    comments: comments.remove(&key).unwrap_or_default(),
                })
            })
            .collect::<Vec<_>>();

        debug!({ count = out.len() }, "found media details in batch");

        Ok(out)
    }

    #[instrument(skip(self))]
    async fn get_media_uuids(&self) -> Result<Vec<MediaUuid>> {
        debug!("getting all media uuids");

        let _mr = self.locks.media.read().await;

        let result = r"
            SELECT media_uuid FROM media"
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let data = result
            .into_iter()
            .map(|row| {
                let input = from_row_opt::<Uuid>(row)?;

                Ok(MediaUuid::from_value(self, input))
            })
            .collect::<Result<Vec<MediaUuid>, FromRowError>>()?;

        debug!({ count = data.len() }, "found media");

        Ok(data)
    }

    #[instrument(skip(self))]
    async fn get_media_by_path(&self, path: String) -> Result<Option<MediaByPath>> {
        debug!("searching for media by path");

        let _mr = self.locks.media.read().await;

        let mut result = r"
            SELECT media_uuid, chash, mtime FROM media WHERE path = :path"
            .with(params! {
                "path" => path,
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let row = match result.pop() {
            Some(row) => row,
            None => return Ok(None),
        };

        let data = from_row_opt::<(Uuid, String, u64)>(row)?;

        debug!({ media_uuid = %data.0 }, "found media");

        Ok(Some(MediaByPath {
            media_uuid: MediaUuid::from_value(self, data.0),
            hash: data.1,
            mtime: data.2,
        }))
    }

    #[instrument(skip(self))]
    async fn get_media_by_chash(
        &self,
        library_uuid: LibraryUuid,
        chash: String,
    ) -> Result<Option<MediaByCHash>> {
        debug!("searching for media by content hash");

        let _mr = self.locks.media.read().await;

        let mut result = r"
            SELECT media_uuid, path, mtime FROM media WHERE library_uuid = :library_uuid AND chash = :chash"
            .with(params! {
                "library_uuid" => library_uuid.value(),
                "chash" => chash,
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let row = match result.pop() {
            Some(row) => row,
            None => return Ok(None),
        };

        let data = from_row_opt::<(Uuid, String, u64)>(row)?;

        debug!({ media_uuid = %data.0 }, "found media");

        Ok(Some(MediaByCHash {
            media_uuid: MediaUuid::from_value(self, data.0),
            path: data.1,
            mtime: data.2,
        }))
    }

    #[instrument(skip(self, update))]
    async fn update_media(&self, media_uuid: MediaUuid, update: MediaUpdate) -> Result<()> {
        debug!("updating media details");

        let _mw = self.locks.media.write().await;

        if let Some(val) = update.hidden {
            r"
            UPDATE media SET hidden = :hidden WHERE media_uuid = :media_uuid"
                .with(params! {
                    "hidden" => val,
                    "media_uuid" => media_uuid.value(),
                })
                .run(self.pool.get_conn().await?)
                .await?;
        }

        if let Some(val) = update.date {
            r"
            UPDATE media SET date = :date WHERE media_uuid = :media_uuid"
                .with(params! {
                    "date" => val.clone(),
                    "media_uuid" => media_uuid.value(),
                })
                .run(self.pool.get_conn().await?)
                .await?;
        }

        if let Some(val) = update.note {
            r"
            UPDATE media SET note = :note WHERE media_uuid = :media_uuid"
                .with(params! {
                    "note" => val.clone(),
                    "media_uuid" => media_uuid.value(),
                })
                .run(self.pool.get_conn().await?)
                .await?;
        }

        if let Some(val) = update.tags {
            r"
            UPDATE media SET tags = :tags WHERE media_uuid = :media_uuid"
                .with(params! {
                    "tags" => fold_set(val.clone())?,
                    "media_uuid" => media_uuid.value(),
                })
                .run(self.pool.get_conn().await?)
                .await?;
        }

        debug!("updated media details");

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

        let _mw = self.locks.media.write().await;

        r"
        UPDATE media SET path = :path, chash = :hash, mtime = :mtime WHERE media_uuid = :media_uuid"
            .with(params! {
                "media_uuid" => media_uuid.value(),
                "path" => path,
                "hash" => hash,
                "mtime" => mtime,
            })
            .run(self.pool.get_conn().await?)
            .await?;

        debug!("replaced media path");

        Ok(())
    }

    #[instrument(skip(self, opts))]
    async fn search_media(
        &self,
        gid: HashSet<String>,
        opts: SearchOptions,
    ) -> Result<SearchPage<MediaUuid>> {
        debug!("searching for media");

        let _mr = self.locks.media.read().await;
        let _lr = self.locks.library.read().await;
        let _xr = self.locks.contents.read().await;
        let _cr = self.locks.collection.read().await;

        // for a given uid and filter, find all non-hidden media that either:
        //  * is in a library owned by a group containing the uid
        //  * is in a collection owned by a group containing the uid
        let gid_match = &*GID_MATCH;
        let base = format!(
            r"
            SELECT
                media.media_uuid
            FROM
                media
            WHERE
                media.hidden = FALSE
                AND (
                    media.library_uuid IN (SELECT library_uuid FROM libraries WHERE {gid_match})
                    OR EXISTS (
                        SELECT 1
                        FROM
                            collection_contents
                            INNER JOIN collections ON collections.collection_uuid = collection_contents.collection_uuid
                        WHERE
                            collection_contents.media_uuid = media.media_uuid
                            AND {gid_match}
                    )
                )"
        );

        let page = self
            .paged_search(&base, vec![("gid", fold_set(gid)?.into())], &opts)
            .await?;

        debug!({ count = page.items.len(), total = page.total }, "found media");

        Ok(page)
    }

    #[instrument(skip(self))]
    async fn similar_media(
        &self,
        gid: HashSet<String>,
        media_uuid: MediaUuid,
        distance: i64,
    ) -> Result<Vec<MediaUuid>> {
        // for a given uid and filter, find all media that match either:
        //  * is in a library owned by a group containing the uid
        //  * if the media is not hidden, is in an collection owned
        //    by a group containing the uid
        debug!("searching for similar media");

        let _mr = self.locks.media.read().await;
        let _lr = self.locks.library.read().await;
        let _xr = self.locks.contents.read().await;
        let _cr = self.locks.collection.read().await;

        let gid_match = &*GID_MATCH;
        let result = format!(
            r"
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
                                {gid_match}
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
                                {gid_match}
                        ) AS t2
                        INNER JOIN media ON t2.library_uuid = media.library_uuid
                ) AS t3
                INNER JOIN media ON t3.media_uuid = media.media_uuid
            WHERE
                media.hidden = FALSE
                AND media.phash != ''
                AND BIG_HAM((SELECT phash FROM media WHERE media_uuid = :media_uuid), media.phash) < :distance"
        )
        .with(params! {
            "gid" => fold_set(gid)?,
            "media_uuid" => media_uuid.value(),
            "distance" => distance,
        })
        .run(self.pool.get_conn().await?)
        .await?
        .collect::<Row>()
        .await?;

        let data = result
            .into_iter()
            .map(|row| {
                let input = from_row_opt::<Uuid>(row)?;

                Ok(MediaUuid::from_value(self, input))
            })
            .collect::<Result<Vec<MediaUuid>, FromRowError>>()?;

        debug!({ count = data.len() }, "found similar media");

        Ok(data)
    }

    // comment queries
    #[instrument(skip(self, comment))]
    async fn add_comment(&self, comment: Comment) -> Result<CommentUuid> {
        debug!({ media_uuid = %comment.media_uuid }, "adding comment");

        let _mw = self.locks.media.write().await;
        let _yw = self.locks.comment.write().await;

        let mut result = r"
            INSERT INTO comments (comment_uuid, media_uuid, uid, date, text)
            VALUES (UUID_v7(), :media_uuid, :uid, :date, :text)
            RETURNING comment_uuid"
            .with(params! {
                "media_uuid" => comment.media_uuid.value(),
                "uid" => comment.uid,
                "date" => SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
                "text" => comment.text,
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let row = result.pop().ok_or_else(|| {
            error!("failed to add comment");
            anyhow::Error::msg("failed to add comment")
        })?;

        let data = from_row_opt::<Uuid>(row)?;

        debug!({media_uuid = %comment.media_uuid, comment_uuid = %data}, "added comment");

        Ok(CommentUuid::from_value(self, data))
    }

    #[instrument(skip(self))]
    async fn get_comment(&self, comment_uuid: CommentUuid) -> Result<Option<Comment>> {
        debug!("getting comment details");

        let _yr = self.locks.comment.read().await;

        let mut result = r"
            SELECT media_uuid, uid, date, text FROM comments WHERE comment_uuid = :comment_uuid"
            .with(params! {
                "comment_uuid" => comment_uuid.value(),
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let row = match result.pop() {
            Some(row) => row,
            None => return Ok(None),
        };

        let data = from_row_opt::<(Uuid, String, u64, String)>(row)?;

        debug!("found comment details");

        Ok(Some(Comment {
            media_uuid: MediaUuid::from_value(self, data.0),
            uid: data.1,
            date: data.2,
            text: data.3,
        }))
    }

    #[instrument(skip(self))]
    async fn get_comment_uuids(&self) -> Result<Vec<CommentUuid>> {
        debug!("getting all comment uuids");

        let _yr = self.locks.comment.read().await;

        let result = r"
            SELECT comment_uuid FROM comments"
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let data = result
            .into_iter()
            .map(|row| {
                let input = from_row_opt::<Uuid>(row)?;

                Ok(CommentUuid::from_value(self, input))
            })
            .collect::<Result<Vec<CommentUuid>, FromRowError>>()?;

        debug!({ count = data.len() }, "found comments");

        Ok(data)
    }

    #[instrument(skip(self))]
    async fn delete_comment(&self, comment_uuid: CommentUuid) -> Result<()> {
        debug!("deleting comment");

        let _yw = self.locks.comment.write().await;

        r"
        DELETE FROM comments WHERE (comment_uuid = :comment_uuid)"
            .with(params! {
                "comment_uuid" => comment_uuid.value(),
            })
            .run(self.pool.get_conn().await?)
            .await?;

        debug!("deleted comment");

        Ok(())
    }

    #[instrument(skip(self, text))]
    async fn update_comment(&self, comment_uuid: CommentUuid, text: Option<String>) -> Result<()> {
        debug!("updating comment");

        let _yw = self.locks.comment.write().await;

        if let Some(val) = text {
            r"
            UPDATE comments SET text = :text WHERE comment_uuid = :comment_uuid"
                .with(params! {
                    "text" => val.clone(),
                    "comment_uuid" => comment_uuid.value(),
                })
                .run(self.pool.get_conn().await?)
                .await?;
        }

        debug!("updated comment");

        Ok(())
    }

    // collection queries
    #[instrument(skip(self, collection))]
    async fn add_collection(&self, collection: Collection) -> Result<CollectionUuid> {
        debug!({ collection_name = collection.name }, "adding collection");

        let _cw = self.locks.collection.write().await;

        let mut result = r"
            INSERT INTO collections (collection_uuid, uid, gid, name, note, tags, cover)
            SELECT
                UUID_v7(),
                :uid,
                :gid,
                :name,
                :note,
                :tags,
                :cover
            FROM
                DUAL
            WHERE NOT EXISTS(
                SELECT 1
                FROM collections
                WHERE
                    uid = :uid
                    AND name = :name
            )
            RETURNING collection_uuid"
            .with(params! {
                "uid" => collection.uid,
                "gid" => collection.gid,
                "name" => collection.name.clone(),
                "note" => collection.note,
                "tags" => fold_set(collection.tags)?,
                "cover" => collection.cover.map(|m| m.value()),
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let row = result.pop().ok_or_else(|| {
            error!("failed to add collection");
            anyhow::Error::msg("failed to add collection")
        })?;

        let data = from_row_opt::<Uuid>(row)?;

        debug!({ collection_name = collection.name, collection_uuid = %data }, "added collection");

        Ok(CollectionUuid::from_value(self, data))
    }

    #[instrument(skip(self))]
    async fn get_collection(&self, collection_uuid: CollectionUuid) -> Result<Option<Collection>> {
        debug!("getting collection details");

        let _cr = self.locks.collection.read().await;

        let mut result = r"
            SELECT uid, gid, name, note, tags, cover FROM collections WHERE collection_uuid = :collection_uuid"
        .with(params! {
            "collection_uuid" => collection_uuid.value(),
        })
        .run(self.pool.get_conn().await?)
        .await?
        .collect::<Row>()
        .await?;

        let row = match result.pop() {
            Some(row) => row,
            None => return Ok(None),
        };

        let data = from_row_opt::<(String, String, String, String, String, Option<Uuid>)>(row)?;

        debug!("found collection details");

        let tags = data.4.clone();

        Ok(Some(Collection {
            uid: data.0,
            gid: data.1,
            name: data.2,
            note: data.3,
            tags: unfold_set(&tags),
            cover: data.5.map(|m| MediaUuid::from_value(self, m)),
        }))
    }

    #[instrument(skip(self))]
    async fn get_collection_uuids(&self) -> Result<Vec<CollectionUuid>> {
        debug!("getting all collection uuids");

        let _xr = self.locks.collection.read().await;

        let result = r"
            SELECT collection_uuid FROM collections"
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let data = result
            .into_iter()
            .map(|row| {
                let input = from_row_opt::<Uuid>(row)?;

                Ok(CollectionUuid::from_value(self, input))
            })
            .collect::<Result<Vec<CollectionUuid>, FromRowError>>()?;

        debug!({ count = data.len() }, "found collections");

        Ok(data)
    }

    #[instrument(skip(self))]
    async fn delete_collection(&self, collection_uuid: CollectionUuid) -> Result<()> {
        debug!("deleting media from collection");

        let _xw = self.locks.contents.write().await;
        let _cw = self.locks.collection.write().await;

        r"
            DELETE FROM collection_contents WHERE collection_uuid = :collection_uuid"
            .with(params! {
                "collection_uuid" => collection_uuid.value(),
            })
            .run(self.pool.get_conn().await?)
            .await?;

        debug!("deleting collection");

        r"
            DELETE FROM collections WHERE collection_uuid = :collection_uuid"
            .with(params! {
                "collection_uuid" => collection_uuid.value(),
            })
            .run(self.pool.get_conn().await?)
            .await?;

        debug!("deleted collection");

        Ok(())
    }

    #[instrument(skip(self, update))]
    async fn update_collection(
        &self,
        collection_uuid: CollectionUuid,
        update: CollectionUpdate,
    ) -> Result<()> {
        debug!("updating collection");

        let _cw = self.locks.collection.write().await;

        if let Some(val) = update.name {
            r"
            UPDATE collections SET name = :name WHERE collection_uuid = :collection_uuid"
                .with(params! {
                    "name" => val.clone(),
                    "collection_uuid" => collection_uuid.value(),
                })
                .run(self.pool.get_conn().await?)
                .await?;
        }

        if let Some(val) = update.note {
            r"
            UPDATE collections SET note = :note WHERE collection_uuid = :collection_uuid"
                .with(params! {
                    "note" => val.clone(),
                    "collection_uuid" => collection_uuid.value(),
                })
                .run(self.pool.get_conn().await?)
                .await?;
        }

        if let Some(val) = update.tags {
            r"
            UPDATE collections SET tags = :tags WHERE collection_uuid = :collection_uuid"
                .with(params! {
                    "tags" => fold_set(val.clone())?,
                    "collection_uuid" => collection_uuid.value(),
                })
                .run(self.pool.get_conn().await?)
                .await?;
        }

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

        let _cw = self.locks.contents.write().await;

        let mut result = r"
            INSERT INTO collection_contents (media_uuid, collection_uuid)
            SELECT
                :media_uuid,
                :collection_uuid
            FROM
                DUAL
            WHERE NOT EXISTS(
                SELECT 1
                FROM collection_contents
                WHERE
                    media_uuid = :media_uuid
                    AND collection_uuid = :collection_uuid
            )
            RETURNING id"
            .with(params! {
                "media_uuid" => media_uuid.value(),
                "collection_uuid" => collection_uuid.value(),
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        result.pop().ok_or_else(|| {
            error!("failed to add media to collection");
            anyhow::Error::msg("failed to add media to collection")
        })?;

        debug!("added media to collection");

        Ok(())
    }

    #[instrument(skip(self))]
    async fn rm_media_from_collection(
        &self,
        media_uuid: MediaUuid,
        collection_uuid: CollectionUuid,
    ) -> Result<()> {
        debug!("removing media from collection");

        let _cw = self.locks.contents.write().await;

        r"
        DELETE FROM collection_contents WHERE (media_uuid = :media_uuid AND collection_uuid = :collection_uuid)"
        .with(params! {
            "media_uuid" => media_uuid.value(),
            "collection_uuid" => collection_uuid.value(),
        })
        .run(self.pool.get_conn().await?)
        .await?;

        debug!("removed media from collection");

        Ok(())
    }

    #[instrument(skip(self, filter))]
    async fn search_collections(
        &self,
        gid: HashSet<String>,
        filter: String,
        page: PageOptions,
    ) -> Result<SearchPage<CollectionUuid>> {
        debug!("searching for collections");

        let _cr = self.locks.collection.read().await;

        let gid_match = &*GID_MATCH;
        let base = format!(
            r"
            SELECT
                collection_uuid
            FROM
                collections
            WHERE
                {gid_match} AND name LIKE :filter"
        );

        let mut params: HashMap<Vec<u8>, Value> = HashMap::from([
            (b"gid".to_vec(), fold_set(gid)?.into()),
            (
                b"filter".to_vec(),
                format!("%{}%", escape_like(&filter)).into(),
            ),
        ]);

        let total = if page.with_total {
            let n = format!("SELECT COUNT(*) FROM ({base}) AS counted")
                .with(Params::Named(params.clone()))
                .first::<u64, _>(self.pool.get_conn().await?)
                .await?
                .unwrap_or(0);
            Some(n)
        } else {
            None
        };

        params.insert(b"limit".to_vec(), page.limit.unwrap_or(u64::MAX).into());
        params.insert(b"offset".to_vec(), page.offset.into());

        let data = format!("{base} ORDER BY name, collection_uuid LIMIT :limit OFFSET :offset")
            .with(Params::Named(params))
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?
            .into_iter()
            .map(|row| {
                let input = from_row_opt::<Uuid>(row)?;

                Ok(CollectionUuid::from_value(self, input))
            })
            .collect::<Result<Vec<CollectionUuid>, FromRowError>>()?;

        debug!({ count = data.len(), total = total }, "found collections");

        Ok(SearchPage { items: data, total })
    }

    #[instrument(skip(self, opts))]
    async fn search_media_in_collection(
        &self,
        gid: HashSet<String>,
        collection_uuid: CollectionUuid,
        opts: SearchOptions,
    ) -> Result<SearchPage<MediaUuid>> {
        debug!("searching media in collection");

        let _mr = self.locks.media.read().await;
        let _xr = self.locks.contents.read().await;
        let _cr = self.locks.collection.read().await;

        // for a given uid, filter, and collection_uuid, find all non-hidden media in that collection
        // provided that the collection is owned by a group containing the uid
        let gid_match = &*GID_MATCH;
        let base = format!(
            r"
            SELECT
                media.media_uuid
            FROM
                collection_contents
                INNER JOIN collections ON collections.collection_uuid = collection_contents.collection_uuid
                INNER JOIN media ON media.media_uuid = collection_contents.media_uuid
            WHERE
                collections.collection_uuid = :collection_uuid
                AND {gid_match}
                AND media.hidden = FALSE"
        );

        let page = self
            .paged_search(
                &base,
                vec![
                    ("gid", fold_set(gid)?.into()),
                    ("collection_uuid", collection_uuid.value().into()),
                ],
                &opts,
            )
            .await?;

        debug!({ count = page.items.len(), total = page.total }, "found media in collection");

        Ok(page)
    }

    // library queries
    #[instrument(skip(self, library))]
    async fn add_library(&self, library: Library) -> Result<LibraryUuid> {
        debug!({ library_path = library.path }, "adding library");

        let _lw = self.locks.library.write().await;

        let mut result = r"
            INSERT INTO libraries (library_uuid, path, gid, count)
            SELECT
                UUID_v7()
                :path,
                :gid,
                :count
            FROM
                DUAL
            WHERE NOT EXISTS(
                SELECT 1
                FROM libraries
                WHERE
                    path = :path
            )
            RETURNING library_uuid"
            .with(params! {
                "path" => library.path.clone(),
                "gid" => library.gid,
                "count" => library.count,
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let row = result.pop().ok_or_else(|| {
            error!("failed to add library");
            anyhow::Error::msg("failed to add library")
        })?;

        let data = from_row_opt::<Uuid>(row)?;

        debug!({ library_path = library.path, library_uuid = %data }, "adding library");

        Ok(LibraryUuid::from_value(self, data))
    }

    #[instrument(skip(self))]
    async fn get_library(&self, library_uuid: LibraryUuid) -> Result<Option<Library>> {
        debug!("getting library details");

        let _lr = self.locks.library.read().await;

        let mut result = r"
            SELECT path, uid, gid, count FROM libraries WHERE library_uuid = :library_uuid"
            .with(params! {
                "library_uuid" => library_uuid.value(),
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let row = match result.pop() {
            Some(row) => row,
            None => return Ok(None),
        };

        let data = from_row_opt::<(String, String, String, i64)>(row)?;

        debug!("found library details");

        Ok(Some(Library {
            path: data.0,
            uid: data.1,
            gid: data.2,
            count: data.3,
        }))
    }

    #[instrument(skip(self))]
    async fn get_library_uuids(&self) -> Result<Vec<LibraryUuid>> {
        debug!("getting all library uuids");

        let _lr = self.locks.library.read().await;

        let result = r"
            SELECT library_uuid FROM libraries"
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let data = result
            .into_iter()
            .map(|row| {
                let input = from_row_opt::<Uuid>(row)?;

                Ok(LibraryUuid::from_value(self, input))
            })
            .collect::<Result<Vec<LibraryUuid>, anyhow::Error>>()?;

        debug!({ count = data.len() }, "found libraries");

        Ok(data)
    }

    #[instrument(skip(self, update))]
    async fn update_library(&self, library_uuid: LibraryUuid, update: LibraryUpdate) -> Result<()> {
        debug!("updating library");

        let _lw = self.locks.library.write().await;

        if let Some(val) = update.count {
            r"
            UPDATE libraries SET count = :count WHERE library_uuid = :library_uuid"
                .with(params! {
                    "count" => val,
                    "library_uuid" => library_uuid.value(),
                })
                .run(self.pool.get_conn().await?)
                .await?;
        }

        debug!("updated library");

        Ok(())
    }

    #[instrument(skip(self, filter))]
    async fn search_libraries(
        &self,
        gid: HashSet<String>,
        filter: String,
    ) -> Result<Vec<LibraryUuid>> {
        debug!("searching libraries");

        let _lr = self.locks.library.read().await;

        let gid_match = &*GID_MATCH;
        let result = format!(
            r"
            SELECT
                library_uuid
            FROM
                libraries
            WHERE
                {gid_match} AND path LIKE :filter"
        )
        .with(params! {
            "gid" => fold_set(gid)?,
            "filter" => format!("%{}%", escape_like(&filter)),
        })
        .run(self.pool.get_conn().await?)
        .await?
        .collect::<Row>()
        .await?;

        let data = result
            .into_iter()
            .map(|row| {
                let input = from_row_opt::<Uuid>(row)?;

                Ok(LibraryUuid::from_value(self, input))
            })
            .collect::<Result<Vec<LibraryUuid>, anyhow::Error>>()?;

        debug!({ count = data.len() }, "found libraries");

        Ok(data)
    }

    #[instrument(skip(self, hidden, opts))]
    async fn search_media_in_library(
        &self,
        gid: HashSet<String>,
        library_uuid: LibraryUuid,
        hidden: Option<bool>,
        opts: SearchOptions,
    ) -> Result<SearchPage<MediaUuid>> {
        debug!("searching media in library");

        let _mr = self.locks.media.read().await;
        let _lr = self.locks.library.read().await;

        let (hidden_sql, hidden) = match hidden {
            Some(v) => ("media.hidden = :hidden", v),
            None => ("TRUE = :hidden", true),
        };

        // for a given uid, filter, hidden state and library_uuid, find all media in that library
        // provided that the library is owned by a group containing the uid
        //
        // note that this is the only search query where media with "hidden = true" can be found
        let gid_match = &*GID_MATCH;
        let base = format!(
            r"
            SELECT
                media.media_uuid
            FROM
                media
            WHERE
                media.library_uuid = :library_uuid
                AND media.library_uuid IN (SELECT library_uuid FROM libraries WHERE {gid_match})
                AND {hidden_sql}"
        );

        let page = self
            .paged_search(
                &base,
                vec![
                    ("gid", fold_set(gid)?.into()),
                    ("library_uuid", library_uuid.value().into()),
                    ("hidden", hidden.into()),
                ],
                &opts,
            )
            .await?;

        debug!({ count = page.items.len(), total = page.total }, "found media in library");

        Ok(page)
    }
}

// exact membership test of the row's gid in the folded set of user groups bound as :gid
//
// the previous INSTR(:gid, gid) was a plain substring match, so membership in "devops"
// would also grant access to "dev"; delimiting both sides with the folding separator (which
// fold_set() guarantees never appears inside a group name) makes it exact.  LOCATE is used
// rather than LIKE so that '%' and '_' in group names are not wildcards.
static GID_MATCH: LazyLock<String> = LazyLock::new(|| {
    // a constant, so it can go into the sql text; the user's groups are still bound as :gid
    let sep = FOLDING_SEPARATOR;
    format!("LOCATE(CONCAT('{sep}', gid, '{sep}'), CONCAT('{sep}', :gid, '{sep}')) > 0")
});

const MEDIA_COLS: &str = "media.path, media.date, media.note, media.tags";

// the columns of a media record, in the order media_from_row expects them
const MEDIA_RECORD_COLS: &str = "media_uuid, library_uuid, path, size, chash, phash, mtime, hidden, date, note, tags, media_type";

// a row selected as MEDIA_RECORD_COLS
fn media_from_row<S: UuidSource>(src: &S, row: Row) -> Result<(Uuid, Media)> {
    let d = from_row_opt::<(
        Uuid,
        Uuid,
        String,
        u64,
        String,
        String,
        u64,
        bool,
        String,
        String,
        String,
        String,
    )>(row)?;

    let media = Media {
        library_uuid: LibraryUuid::from_value(src, d.1),
        path: d.2,
        size: d.3,
        chash: d.4,
        phash: d.5,
        mtime: d.6,
        hidden: d.7,
        date: d.8,
        note: d.9,
        tags: unfold_set(&d.10),
        metadata: parse_media_type(d.0, &d.11)?,
    };

    Ok((d.0, media))
}

fn parse_media_type(media_uuid: Uuid, s: &str) -> Result<MediaMetadata> {
    match s {
        "Image" => Ok(MediaMetadata::Image),
        "Video" => Ok(MediaMetadata::Video),
        "VideoSlice" => Ok(MediaMetadata::VideoSlice),
        "Audio" => Ok(MediaMetadata::Audio),
        _ => {
            error!("invalid media record");
            Err(anyhow::Error::msg(format!(
                "invalid media record for {media_uuid}"
            )))
        }
    }
}

impl MariaDBBackend {
    // run a "SELECT media_uuid, x_uuid ... WHERE media_uuid IN (?, ...)" query with the
    // media uuids bound positionally, and group the x_uuids by media
    async fn group_by_media<T>(
        &self,
        statement: String,
        values: Vec<Uuid>,
        convert: impl Fn(Uuid) -> T,
    ) -> Result<HashMap<Uuid, Vec<T>>> {
        let mut grouped: HashMap<Uuid, Vec<T>> = HashMap::new();

        for row in statement
            .with(values)
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?
        {
            let (media_uuid, x) = from_row_opt::<(Uuid, Uuid)>(row)?;
            grouped.entry(media_uuid).or_default().push(convert(x));
        }

        Ok(grouped)
    }

    // run a paged media search
    //
    // base must be a SELECT of a single media_uuid column over a FROM clause that includes
    // the media table, ending inside its WHERE clause; the filter, ORDER BY and LIMIT/OFFSET
    // are appended with all user input bound as the named parameters :filter, :limit and
    // :offset.  when opts.with_total is set, a separate COUNT(*) is run without ORDER/LIMIT.
    async fn paged_search(
        &self,
        base: &str,
        base_params: Vec<(&str, Value)>,
        opts: &SearchOptions,
    ) -> Result<SearchPage<MediaUuid>> {
        let q = opts.mariadb_sql(MEDIA_COLS);

        let mut params: HashMap<Vec<u8>, Value> = base_params
            .into_iter()
            .map(|(k, v)| (k.as_bytes().to_vec(), v))
            .collect();
        params.insert(b"filter".to_vec(), q.filter_param.clone().into());

        let total = if opts.with_total {
            let count = format!("SELECT COUNT(*) FROM ({base}{}) AS counted", q.filter_sql);
            let n = count
                .with(Params::Named(params.clone()))
                .first::<u64, _>(self.pool.get_conn().await?)
                .await?
                .unwrap_or(0);
            Some(n)
        } else {
            None
        };

        params.insert(b"limit".to_vec(), q.limit.into());
        params.insert(b"offset".to_vec(), q.offset.into());

        let statement = format!("{base}{}{}{}", q.filter_sql, q.order_sql, q.page_sql);

        let items = statement
            .with(Params::Named(params))
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?
            .into_iter()
            .map(|row| Ok(MediaUuid::from_value(self, from_row_opt::<Uuid>(row)?)))
            .collect::<Result<Vec<MediaUuid>, FromRowError>>()?;

        Ok(SearchPage { items, total })
    }
}
