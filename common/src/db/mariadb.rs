use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use chrono::Local;
use mysql_async::{FromRowError, Pool, Row, from_row_opt, prelude::*};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info, instrument};

use crate::{config::ESConfig, db::DbBackend};
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
pub struct MariaDbConfig {
    pub url: String,
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

#[async_trait]
impl DbBackend for MariaDBBackend {
    fn new(config: Arc<ESConfig>) -> Result<Self> {
        info!("creating MariaDB connection pool");

        let url = config
            .mariadb
            .clone()
            .expect("mariadb.url config not present")
            .url;

        Ok(Self {
            pool: Pool::new(url.as_str()),
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
            /* media_access_groups {uuid} */
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
                "media_uuid" => media_uuid,
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
                UUID_SHORT(),
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
                "library_uuid" => media.library_uuid,
                "path" => media.path.clone(),
                "size" => media.size,
                "chash" => media.chash,
                "phash" => media.phash,
                "mtime" => Local::now().timestamp(),
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

        let row = result
            .pop()
            .ok_or_else(|| anyhow::Error::msg("failed to add media"))?;

        let data = from_row_opt::<MediaUuid>(row)?;

        debug!({ media_path = media.path, media_uuid = data }, "added media");

        Ok(data)
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

        let mut media_result = r"
            SELECT library_uuid, path, size, chash, phash, mtime, hidden, date, note, tags, media_type FROM media WHERE media_uuid = :media_uuid"
        .with(params! {
            "media_uuid" => media_uuid,
        })
        .run(self.pool.get_conn().await?)
        .await?
        .collect::<Row>()
        .await?;

        let media_data = match media_result.pop() {
            Some(row) => from_row_opt::<(
                LibraryUuid,
                String,
                u64,
                String,
                String,
                i64,
                bool,
                String,
                String,
                String,
                String,
            )>(row)?,
            None => return Ok(None),
        };

        let collection_result = r"
            SELECT collection_uuid FROM collection_contents WHERE media_uuid = :media_uuid"
            .with(params! {
                "media_uuid" => media_uuid,
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let collection_data = collection_result
            .into_iter()
            .map(from_row_opt::<CollectionUuid>)
            .collect::<Result<Vec<_>, FromRowError>>()?;

        let comment_result = r"
            SELECT comment_uuid FROM comments WHERE media_uuid = :media_uuid"
            .with(params! {
                "media_uuid" => media_uuid,
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let comment_data = comment_result
            .into_iter()
            .map(from_row_opt::<CommentUuid>)
            .collect::<Result<Vec<_>, FromRowError>>()?;

        debug!("found media details");

        Ok(Some((
            Media {
                library_uuid: media_data.0,
                path: media_data.1,
                size: media_data.2,
                chash: media_data.3,
                phash: media_data.4,
                mtime: media_data.5,
                hidden: media_data.6,
                date: media_data.7,
                note: media_data.8,
                tags: unfold_set(&media_data.9),
                metadata: match media_data.10.as_str() {
                    "Image" => MediaMetadata::Image,
                    "Video" => MediaMetadata::Video,
                    "VideoSlice" => MediaMetadata::VideoSlice,
                    "Audio" => MediaMetadata::Audio,
                    _ => {
                        return Err(anyhow::Error::msg(format!(
                            "invalid media record for {media_uuid}"
                        )));
                    }
                },
            },
            collection_data,
            comment_data,
        )))
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
            .map(from_row_opt::<MediaUuid>)
            .collect::<Result<Vec<_>, FromRowError>>()?;

        debug!({ count = data.len() }, "found media");

        Ok(data)
    }

    #[instrument(skip(self))]
    async fn get_media_uuid_by_path(&self, path: String) -> Result<Option<MediaUuid>> {
        debug!("searching for media by path");

        let _mr = self.locks.media.read().await;

        let mut result = r"
            SELECT media_uuid FROM media WHERE path = :path"
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

        let data = from_row_opt::<MediaUuid>(row)?;

        debug!({ media_uuid = data }, "found media");

        Ok(Some(data))
    }

    #[instrument(skip(self))]
    async fn get_media_uuid_by_chash(
        &self,
        library_uuid: LibraryUuid,
        chash: String,
    ) -> Result<Option<MediaUuid>> {
        debug!("searching for media by content hash");

        let _mr = self.locks.media.read().await;

        let mut result = r"
            SELECT media_uuid FROM media WHERE library_uuid = :library_uuid AND chash = :chash"
            .with(params! {
                "library_uuid" => library_uuid,
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

        let data = from_row_opt::<MediaUuid>(row)?;

        debug!({ media_uuid = data }, "found media");

        Ok(Some(data))
    }

    #[instrument(skip(self, update))]
    async fn update_media(&self, media_uuid: MediaUuid, update: MediaUpdate) -> Result<()> {
        debug!("updating media details");

        let _mw = self.locks.media.write().await;

        let mut modified = false;

        if let Some(val) = update.hidden {
            r"
            UPDATE media SET hidden = :hidden WHERE media_uuid = :media_uuid"
                .with(params! {
                    "hidden" => val,
                    "media_uuid" => media_uuid,
                })
                .run(self.pool.get_conn().await?)
                .await?;

            modified = true;
        }

        if let Some(val) = update.date {
            r"
            UPDATE media SET date = :date WHERE media_uuid = :media_uuid"
                .with(params! {
                    "date" => val.clone(),
                    "media_uuid" => media_uuid,
                })
                .run(self.pool.get_conn().await?)
                .await?;

            modified = true;
        }

        if let Some(val) = update.note {
            r"
            UPDATE media SET note = :note WHERE media_uuid = :media_uuid"
                .with(params! {
                    "note" => val.clone(),
                    "media_uuid" => media_uuid,
                })
                .run(self.pool.get_conn().await?)
                .await?;

            modified = true;
        }

        if let Some(val) = update.tags {
            r"
            UPDATE media SET tags = :tags WHERE media_uuid = :media_uuid"
                .with(params! {
                    "tags" => fold_set(val.clone())?,
                    "media_uuid" => media_uuid,
                })
                .run(self.pool.get_conn().await?)
                .await?;

            modified = true;
        }

        if modified {
            r"
            UPDATE media SET mtime = :mtime WHERE media_uuid = :media_uuid"
                .with(params! {
                    "mtime" => Local::now().timestamp(),
                    "media_uuid" => media_uuid,
                })
                .run(self.pool.get_conn().await?)
                .await?;
        }

        debug!({ modified = modified }, "updated media details");

        Ok(())
    }

    #[instrument(skip(self))]
    async fn replace_media_path(&self, media_uuid: MediaUuid, path: String) -> Result<()> {
        debug!("replacing media path");

        let _mw = self.locks.media.write().await;

        r"
        UPDATE media SET path = :path, mtime = :mtime WHERE media_uuid = :media_uuid"
            .with(params! {
                "mtime" => Local::now().timestamp(),
                "path" => path,
                "media_uuid" => media_uuid,
            })
            .run(self.pool.get_conn().await?)
            .await?;

        debug!("replaced media path");

        Ok(())
    }

    #[instrument(skip(self))]
    async fn search_media(
        &self,
        gid: HashSet<String>,
        filter: SearchFilter,
    ) -> Result<Vec<MediaUuid>> {
        debug!("searching for media");

        let _mr = self.locks.media.read().await;
        let _lr = self.locks.library.read().await;
        let _xr = self.locks.contents.read().await;
        let _cr = self.locks.collection.read().await;

        let (sql, filter) = filter.format_mariadb("media.path, media.date, media.note, media.tags");

        // for a given uid and filter, find all media that match either:
        //  * is in a library owned by a group containing the uid
        //  * if the media is not hidden, is in an collection owned
        //    by a group containing the uid
        let mut query = r"
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
                                INSTR(:gid, gid) > 0
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
                                INSTR(:gid, gid) > 0
                        ) AS t2
                        INNER JOIN media ON t2.library_uuid = media.library_uuid
                ) AS t3
                INNER JOIN media ON t3.media_uuid = media.media_uuid
            WHERE
                media.hidden = FALSE".to_owned();

        query.push_str(&sql);

        let result = query
            .with(params! {
                "gid" => fold_set(gid)?,
                "filter" => filter,
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let data = result
            .into_iter()
            .map(from_row_opt::<MediaUuid>)
            .collect::<Result<Vec<_>, FromRowError>>()?;

        debug!({ count = data.len() }, "found media");

        Ok(data)
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

        let result = r"
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
                                INSTR(:gid, gid) > 0
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
                                INSTR(:gid, gid) > 0
                        ) AS t2
                        INNER JOIN media ON t2.library_uuid = media.library_uuid
                ) AS t3
                INNER JOIN media ON t3.media_uuid = media.media_uuid
            WHERE
                media.hidden = FALSE
                AND media.phash != ''
                AND BIG_HAM((SELECT phash FROM media WHERE media_uuid = :media_uuid), media.phash) < :distance"
        .with(params! {
            "gid" => fold_set(gid)?,
            "media_uuid" => media_uuid,
            "distance" => distance,
        })
        .run(self.pool.get_conn().await?)
        .await?
        .collect::<Row>()
        .await?;

        let data = result
            .into_iter()
            .map(from_row_opt::<MediaUuid>)
            .collect::<Result<Vec<_>, FromRowError>>()?;

        debug!({ count = data.len() }, "found similar media");

        Ok(data)
    }

    // collection queries
    #[instrument(skip(self, collection))]
    async fn add_collection(&self, collection: Collection) -> Result<CollectionUuid> {
        debug!({ collection_name = collection.name }, "adding collection");

        let _cw = self.locks.collection.write().await;

        let mut result = r"
            INSERT INTO collections (collection_uuid, uid, gid, mtime, name, note, tags, cover)
            SELECT
                UUID_SHORT(),
                :uid,
                :gid,
                :mtime,
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
                "mtime" => Local::now().timestamp(),
                "name" => collection.name.clone(),
                "note" => collection.note,
                "tags" => fold_set(collection.tags)?,
                "cover" => collection.cover,
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let row = result
            .pop()
            .ok_or_else(|| anyhow::Error::msg("failed to add collection to the database"))?;

        let data = from_row_opt::<CollectionUuid>(row)?;

        debug!({ collection_name = collection.name, collection_uuid = data }, "added collection");

        Ok(data)
    }

    #[instrument(skip(self))]
    async fn get_collection(&self, collection_uuid: CollectionUuid) -> Result<Option<Collection>> {
        debug!("getting collection details");

        let _cr = self.locks.collection.read().await;

        let mut result = r"
            SELECT uid, gid, mtime, name, note, tags, cover FROM collections WHERE collection_uuid = :collection_uuid"
        .with(params! {
            "collection_uuid" => collection_uuid,
        })
        .run(self.pool.get_conn().await?)
        .await?
        .collect::<Row>()
        .await?;

        let row = match result.pop() {
            Some(row) => row,
            None => return Ok(None),
        };

        let data = from_row_opt::<(
            String,
            String,
            i64,
            String,
            String,
            String,
            Option<MediaUuid>,
        )>(row)?;

        debug!("found collection details");

        Ok(Some(Collection {
            uid: data.0,
            gid: data.1,
            mtime: data.2,
            name: data.3,
            note: data.4,
            tags: unfold_set(&data.5),
            cover: data.6,
        }))
    }

    #[instrument(skip(self))]
    async fn delete_collection(&self, collection_uuid: CollectionUuid) -> Result<()> {
        debug!("deleting media from collection");

        let _xw = self.locks.contents.write().await;
        let _cw = self.locks.collection.write().await;

        r"
            DELETE FROM collection_contents WHERE collection_uuid = :collection_uuid"
            .with(params! {
                "collection_uuid" => collection_uuid,
            })
            .run(self.pool.get_conn().await?)
            .await?;

        debug!("deleting collection");

        r"
            DELETE FROM collections WHERE collection_uuid = :collection_uuid"
            .with(params! {
                "collection_uuid" => collection_uuid,
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

        let mut modified = false;

        if let Some(val) = update.name {
            r"
            UPDATE collections SET name = :name WHERE collection_uuid = :collection_uuid"
                .with(params! {
                    "name" => val.clone(),
                    "collection_uuid" => collection_uuid,
                })
                .run(self.pool.get_conn().await?)
                .await?;

            modified = true;
        }

        if let Some(val) = update.note {
            r"
            UPDATE collections SET note = :note WHERE collection_uuid = :collection_uuid"
                .with(params! {
                    "note" => val.clone(),
                    "collection_uuid" => collection_uuid,
                })
                .run(self.pool.get_conn().await?)
                .await?;

            modified = true;
        }

        if let Some(val) = update.tags {
            r"
            UPDATE collections SET tags = :tags WHERE collection_uuid = :collection_uuid"
                .with(params! {
                    "tags" => fold_set(val.clone())?,
                    "collection_uuid" => collection_uuid,
                })
                .run(self.pool.get_conn().await?)
                .await?;

            modified = true;
        }

        if modified {
            r"
            UPDATE collections SET mtime = :mtime WHERE collection_uuid = :collection_uuid"
                .with(params! {
                    "mtime" => Local::now().timestamp(),
                    "collection_uuid" => collection_uuid,
                })
                .run(self.pool.get_conn().await?)
                .await?;
        }

        debug!({ modified = modified }, "updated collection");

        Ok(())
    }

    #[instrument(skip(self))]
    async fn add_media_to_collection(
        &self,
        media_uuid: MediaUuid,
        collection_uuid: CollectionUuid,
    ) -> Result<()> {
        debug!("adding media to collection");

        let _mw = self.locks.media.write().await;
        let _cw = self.locks.collection.write().await;

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
                "media_uuid" => media_uuid,
                "collection_uuid" => collection_uuid,
                "mtime" => Local::now().timestamp(),
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        result
            .pop()
            .ok_or_else(|| anyhow::Error::msg("failed to add media to collection"))?;

        r"
        UPDATE collections SET mtime = :mtime WHERE collection_uuid = :collection_uuid"
            .with(params! {
                "mtime" => Local::now().timestamp(),
                "collection_uuid" => collection_uuid,
            })
            .run(self.pool.get_conn().await?)
            .await?;

        r"
        UPDATE media SET mtime = :mtime WHERE media_uuid = :media_uuid"
            .with(params! {
                "mtime" => Local::now().timestamp(),
                "media_uuid" => media_uuid,
            })
            .run(self.pool.get_conn().await?)
            .await?;

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

        let _mw = self.locks.media.write().await;
        let _cw = self.locks.collection.write().await;

        r"
        DELETE FROM collection_contents WHERE (media_uuid = :media_uuid AND collection_uuid = :collection_uuid)"
        .with(params! {
            "media_uuid" => media_uuid,
            "collection_uuid" => collection_uuid,
            "mtime" => Local::now().timestamp(),
        })
        .run(self.pool.get_conn().await?)
        .await?;

        r"
        UPDATE collections SET mtime = :mtime WHERE collection_uuid = :collection_uuid"
            .with(params! {
                "mtime" => Local::now().timestamp(),
                "collection_uuid" => collection_uuid,
            })
            .run(self.pool.get_conn().await?)
            .await?;

        r"
        UPDATE media SET mtime = :mtime WHERE media_uuid = :media_uuid"
            .with(params! {
                "mtime" => Local::now().timestamp(),
                "media_uuid" => media_uuid,
            })
            .run(self.pool.get_conn().await?)
            .await?;

        debug!("removed media from collection");

        Ok(())
    }

    #[instrument(skip(self))]
    async fn search_collections(
        &self,
        gid: HashSet<String>,
        filter: SearchFilter,
    ) -> Result<Vec<CollectionUuid>> {
        debug!("searching for collections");

        let _cr = self.locks.collection.read().await;

        let (sql, filter) =
            filter.format_mariadb("collections.name, collections.note, collections.tags");

        // for a given uid and filter, find all collections owned by groups that contain that uid
        let mut query = r"
            SELECT
                collection_uuid
            FROM
                collections
            WHERE
                INSTR(:gid, gid) > 0"
            .to_owned();

        query.push_str(&sql);

        let result = query
            .with(params! {
                "gid" => fold_set(gid)?,
                "filter" => filter,
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let data = result
            .into_iter()
            .map(from_row_opt::<CollectionUuid>)
            .collect::<Result<Vec<_>, FromRowError>>()?;

        debug!({ count = data.len() }, "found collections");

        Ok(data)
    }

    #[instrument(skip(self))]
    async fn search_media_in_collection(
        &self,
        gid: HashSet<String>,
        collection_uuid: CollectionUuid,
        filter: SearchFilter,
    ) -> Result<Vec<MediaUuid>> {
        debug!("searching media in collection");

        let _mr = self.locks.media.read().await;
        let _xr = self.locks.contents.read().await;
        let _cr = self.locks.collection.read().await;

        let (sql, filter) = filter.format_mariadb("media.path, media.date, media.note, media.tags");

        // for a given uid, filter, and collection_uuid, find all non-hidden media in that collection
        // provided that the collection is owned by a group containing the uid
        let mut query = r"
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
                                INSTR(:gid, gid) > 0 AND collection_uuid = :collection_uuid
                        ) AS t2
                        INNER JOIN collection_contents ON t2.collection_uuid = collection_contents.collection_uuid
                ) AS t3
                INNER JOIN media ON t3.media_uuid = media.media_uuid
            WHERE
                media.hidden = FALSE".to_owned();

        query.push_str(&sql);

        let result = query
            .with(params! {
                "gid" => fold_set(gid)?,
                "collection_uuid" => collection_uuid,
                "filter" => filter,
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let data = result
            .into_iter()
            .map(from_row_opt::<MediaUuid>)
            .collect::<Result<Vec<_>, FromRowError>>()?;

        debug!({ count = data.len() }, "found media in collection");

        Ok(data)
    }

    // library queries
    #[instrument(skip(self, library))]
    async fn add_library(&self, library: Library) -> Result<LibraryUuid> {
        // since we require that libraries have unique paths, it might seem like we want
        // to use that as the primary key.  but those strings might be arbitrarily complex,
        // so instead using an i64 as a handle is much simpler
        debug!({ library_path = library.path }, "adding library");

        let _lw = self.locks.library.write().await;

        let mut result = r"
            INSERT INTO libraries (library_uuid, path, gid, mtime, count)
            SELECT
                UUID_SHORT()
                :path,
                :gid,
                :mtime,
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
                "mtime" => Local::now().timestamp(),
                "count" => library.count,
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let row = result
            .pop()
            .ok_or_else(|| anyhow::Error::msg("failed to add library to the database"))?;

        let data = from_row_opt::<LibraryUuid>(row)?;

        debug!({ library_path = library.path, library_uuid = data }, "adding library");

        Ok(data)
    }

    #[instrument(skip(self))]
    async fn get_library(&self, library_uuid: LibraryUuid) -> Result<Option<Library>> {
        debug!("getting library details");

        let _lr = self.locks.library.read().await;

        let mut result = r"
            SELECT path, uid, gid, mtime, count FROM libraries WHERE library_uuid = :library_uuid"
            .with(params! {
                "library_uuid" => library_uuid,
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let row = match result.pop() {
            Some(row) => row,
            None => return Ok(None),
        };

        let data = from_row_opt::<(String, String, String, i64, i64)>(row)?;

        debug!("found library details");

        Ok(Some(Library {
            path: data.0,
            uid: data.1,
            gid: data.2,
            mtime: data.3,
            count: data.4,
        }))
    }

    #[instrument(skip(self, update))]
    async fn update_library(&self, library_uuid: LibraryUuid, update: LibraryUpdate) -> Result<()> {
        debug!("updating library");

        let _lw = self.locks.library.write().await;

        let mut modified = false;

        if let Some(val) = update.count {
            r"
            UPDATE libraries SET mtime = :mtime, count = :count WHERE library_uuid = :library_uuid"
                .with(params! {
                    "mtime" => Local::now().timestamp(),
                    "count" => val,
                    "library_uuid" => library_uuid,
                })
                .run(self.pool.get_conn().await?)
                .await?;

            modified = true;
        }

        debug!({ modified = modified }, "updated library");

        Ok(())
    }

    #[instrument(skip(self))]
    async fn search_libraries(
        &self,
        gid: HashSet<String>,
        filter: String,
    ) -> Result<Vec<LibraryUuid>> {
        debug!("searching libraries");

        let _lr = self.locks.library.read().await;

        let result = r"
            SELECT
                library_uuid
            FROM
                libraries
            WHERE
                INSTR(:gid, gid) > 0 AND path LIKE :filter"
            .with(params! {
                "gid" => fold_set(gid)?,
                "filter" => format!("%{}%", filter),
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let data = result
            .into_iter()
            .map(from_row_opt::<LibraryUuid>)
            .collect::<Result<Vec<_>, FromRowError>>()?;

        debug!({ count = data.len() }, "found libraries");

        Ok(data)
    }

    #[instrument(skip(self))]
    async fn search_media_in_library(
        &self,
        gid: HashSet<String>,
        library_uuid: LibraryUuid,
        hidden: Option<bool>,
        filter: SearchFilter,
    ) -> Result<Vec<MediaUuid>> {
        debug!("searching media in library");

        let _mr = self.locks.media.read().await;
        let _lr = self.locks.library.read().await;

        let (filter_sql, filter) =
            filter.format_mariadb("media.path, media.date, media.note, media.tags");

        let (hidden_sql, hidden) = match hidden {
            Some(v) => (String::from("media.hidden = :hidden"), v),
            None => (String::from("true = :hidden"), true),
        };

        // for a given uid, filter, hidden state and library_uuid, find all media in that collection
        // provided that the library is owned by a group containing the uid
        //
        // note that this is the only search query where media with "hidden = true" can be found
        let mut query = r"
            SELECT
                media.media_uuid
            FROM
                (
                    SELECT
                        library_uuid
                    FROM
                        libraries
                    WHERE
                        INSTR(:gid, gid) > 0 AND library_uuid = :library_uuid
                ) AS t1
                INNER JOIN media ON t1.library_uuid = media.library_uuid
            WHERE
            "
        .to_owned();

        query.push_str(&hidden_sql);

        query.push_str(&filter_sql);

        let result = query
            .with(params! {
                "gid" => fold_set(gid)?,
                "library_uuid" => library_uuid,
                "hidden" => hidden,
                "filter" => filter,
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let data = result
            .into_iter()
            .map(from_row_opt::<MediaUuid>)
            .collect::<Result<Vec<_>, FromRowError>>()?;

        debug!({ count = data.len() }, "found media in library");

        Ok(data)
    }

    // comment queries
    #[instrument(skip(self, comment))]
    async fn add_comment(&self, comment: Comment) -> Result<CommentUuid> {
        debug!({ media_uuid = comment.media_uuid }, "adding comment");

        let _mw = self.locks.media.write().await;
        let _yw = self.locks.comment.write().await;

        let mut result = r"
            INSERT INTO comments (comment_uuid, media_uuid, mtime, uid, text)
            VALUES (UUID_SHORT(), :media_uuid, :mtime, :uid, :text)
            RETURNING comment_uuid"
            .with(params! {
                "media_uuid" => comment.media_uuid,
                "mtime" => Local::now().timestamp(),
                "uid" => comment.uid,
                "text" => comment.text,
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let row = result
            .pop()
            .ok_or_else(|| anyhow::Error::msg("failed to add comment to database"))?;

        let data = from_row_opt::<CommentUuid>(row)?;

        r"
        UPDATE media SET mtime = :mtime WHERE media_uuid = :media_uuid"
            .with(params! {
                "mtime" => Local::now().timestamp(),
                "media_uuid" => comment.media_uuid,
            })
            .run(self.pool.get_conn().await?)
            .await?;

        debug!({media_uuid = comment.media_uuid, comment_uuid = data}, "added comment");

        Ok(data)
    }

    #[instrument(skip(self))]
    async fn get_comment(&self, comment_uuid: CommentUuid) -> Result<Option<Comment>> {
        debug!("getting comment details");

        let _yr = self.locks.comment.read().await;

        let mut result = r"
            SELECT media_uuid, mtime, uid, text FROM comments WHERE comment_uuid = :comment_uuid"
            .with(params! {
                "comment_uuid" => comment_uuid,
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let row = match result.pop() {
            Some(row) => row,
            None => return Ok(None),
        };

        let data = from_row_opt::<(MediaUuid, i64, String, String)>(row)?;

        debug!("found comment details");

        Ok(Some(Comment {
            media_uuid: data.0,
            mtime: data.1,
            uid: data.2,
            text: data.3,
        }))
    }

    // TODO -- find a simple way to get the media_uuid so that we can bump the timestamp

    #[instrument(skip(self))]
    async fn delete_comment(&self, comment_uuid: CommentUuid) -> Result<()> {
        debug!("deleting comment");

        // let _mw = self.locks.media.write().await;
        let _yw = self.locks.comment.write().await;

        r"
        DELETE FROM comments WHERE (comment_uuid = :comment_uuid)"
            .with(params! {
                "comment_uuid" => comment_uuid,
            })
            .run(self.pool.get_conn().await?)
            .await?;

        // r"
        // UPDATE media SET mtime = :mtime WHERE media_uuid = :media_uuid"
        //     .with(params! {
        //         "mtime" => Local::now().timestamp(),
        //         "media_uuid" => comment.media_uuid,
        //     })
        //     .run(self.pool.get_conn().await?)
        //     .await?;

        debug!("deleted comment");

        Ok(())
    }

    #[instrument(skip(self, text))]
    async fn update_comment(&self, comment_uuid: CommentUuid, text: Option<String>) -> Result<()> {
        debug!("updating comment");

        // let _mw = self.locks.media.write().await;
        let _yw = self.locks.comment.write().await;

        if let Some(val) = text {
            r"
            UPDATE comments SET text = :text WHERE comment_uuid = :comment_uuid"
                .with(params! {
                    "text" => val.clone(),
                    "comment_uuid" => comment_uuid,
                })
                .run(self.pool.get_conn().await?)
                .await?;

            // r"
            // UPDATE media SET mtime = :mtime WHERE media_uuid = :media_uuid"
            // .with(params! {
            //     "mtime" => Local::now().timestamp(),
            //     "media_uuid" => comment.media_uuid,
            // })
            // .run(self.pool.get_conn().await?)
            // .await?;
        }

        debug!("updated comment");

        Ok(())
    }
}
