use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use chrono::Local;
use mysql_async::{from_row_opt, prelude::*, FromRowError, Pool, Row};
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

pub struct MariaDBBackend {
    pool: Pool,
}

#[async_trait]
impl DbBackend for MariaDBBackend {
    fn new(config: Arc<ESConfig>) -> Result<Self> {
        info!("creating MariaDB connection pool");

        Ok(Self {
            pool: Pool::new(config.mariadb_url.clone().as_str()),
        })
    }

    #[instrument(skip_all)]
    async fn media_access_groups(&self, media_uuid: MediaUuid) -> anyhow::Result<HashSet<String>> {
        debug!({ media_uuid = media_uuid }, "finding media access groups");

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
                "media_uuid" => media_uuid,
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let data = result
            .into_iter()
            .map(|row| from_row_opt::<String>(row))
            .collect::<Result<HashSet<_>, FromRowError>>()?;

        debug!({media_uuid = media_uuid, groups = ?data}, "found groups");

        Ok(data)
    }

    // media queries
    #[instrument(skip_all)]
    async fn add_media(&self, media: Media) -> anyhow::Result<MediaUuid> {
        debug!({ media_path = media.path }, "adding media");

        let mut result = r"
            INSERT INTO media (media_uuid, library_uuid, path, hash, mtime, hidden, date, note, media_type)
            SELECT
                UUID_SHORT(),
                :library_uuid,
                :path,
                :hash,
                :mtime,
                :hidden,
                :date,
                :note,
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
            RETURNING media_uuid"
        .with(params! {
            "library_uuid" => media.library_uuid,
            "path" => media.path.clone(),
            "hash" => media.hash,
            "mtime" => Local::now().timestamp(),
            "hidden" => media.hidden,
            "date" => media.date,
            "note" => media.note,
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

        debug!({media_path = media.path, media_uuid = data}, "added media");

        Ok(data)
    }

    #[instrument(skip_all)]
    async fn get_media(
        &self,
        media_uuid: MediaUuid,
    ) -> anyhow::Result<Option<(Media, Vec<CollectionUuid>, Vec<CommentUuid>)>> {
        debug!({ media_uuid = media_uuid }, "getting media details");

        let mut media_result = r"
            SELECT library_uuid, path, hash, mtime, hidden, date, note, tags, media_type FROM media WHERE media_uuid = :media_uuid"
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
            .map(|row| from_row_opt::<CollectionUuid>(row))
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
            .map(|row| from_row_opt::<CommentUuid>(row))
            .collect::<Result<Vec<_>, FromRowError>>()?;

        debug!({ media_uuid = media_uuid }, "found media details");

        Ok(Some((
            Media {
                library_uuid: media_data.0,
                path: media_data.1,
                hash: media_data.2,
                mtime: media_data.3,
                hidden: media_data.4,
                date: media_data.5,
                note: media_data.6,
                tags: unfold_set(&media_data.7),
                metadata: match media_data.8.as_str() {
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

    #[instrument(skip_all)]
    async fn get_media_uuid_by_path(&self, path: String) -> anyhow::Result<Option<MediaUuid>> {
        debug!({ media_path = path }, "searching for media by path");

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

        Ok(Some(data))
    }

    #[instrument(skip_all)]
    async fn update_media(&self, media_uuid: MediaUuid, update: MediaUpdate) -> anyhow::Result<()> {
        debug!({ media_uuid = media_uuid }, "updating media details");

        if let Some(val) = update.hidden {
            r"
            UPDATE media SET hidden = :hidden WHERE media_uuid = :media_uuid"
                .with(params! {
                    "hidden" => val,
                    "media_uuid" => media_uuid,
                })
                .run(self.pool.get_conn().await?)
                .await?;
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
        }

        r"
        UPDATE media SET mtime = :mtime WHERE media_uuid = :media_uuid"
            .with(params! {
                "mtime" => Local::now().timestamp(),
                "media_uuid" => media_uuid,
            })
            .run(self.pool.get_conn().await?)
            .await?;

        debug!({ media_uuid = media_uuid }, "updated media details");

        Ok(())
    }

    #[instrument(skip_all)]
    async fn search_media(
        &self,
        gid: HashSet<String>,
        filter: SearchFilter,
    ) -> anyhow::Result<Vec<MediaUuid>> {
        let (sql, filter) = filter.format_mariadb("media.path, media.date, media.note, media.tags");

        // for a given uid and filter, find all media that match either:
        //  * is in a library owned by a group containing the uid
        //  * if the media is not hidden, is in an collection owned
        //    by a group containing the uid
        let mut query  = r"
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
            .map(|row| from_row_opt::<MediaUuid>(row))
            .collect::<Result<Vec<_>, FromRowError>>()?;

        Ok(data)
    }

    #[instrument(skip_all)]
    async fn similar_media(
        &self,
        gid: HashSet<String>,
        media_uuid: MediaUuid,
        distance: i64,
    ) -> anyhow::Result<Vec<MediaUuid>> {
        // for a given uid and filter, find all media that match either:
        //  * is in a library owned by a group containing the uid
        //  * if the media is not hidden, is in an collection owned
        //    by a group containing the uid
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
                AND media.hash != ''
                AND BIG_HAM((SELECT hash FROM media WHERE media_uuid = :media_uuid), media.hash) < :distance"
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
            .map(|row| from_row_opt::<MediaUuid>(row))
            .collect::<Result<Vec<_>, FromRowError>>()?;

        Ok(data)
    }

    // collection queries
    #[instrument(skip_all)]
    async fn add_collection(&self, collection: Collection) -> anyhow::Result<CollectionUuid> {
        debug!({ collection_name = collection.name }, "adding collection");

        let mut result = r"
            INSERT INTO collections (collection_uuid, uid, gid, mtime, name, note, tags)
            SELECT
                UUID_SHORT(),
                :uid,
                :gid,
                :mtime,
                :name,
                :note,
                :tags
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
                "tags" => fold_set(collection.tags)?
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let row = result
            .pop()
            .ok_or_else(|| anyhow::Error::msg("failed to add collection to the database"))?;

        let data = from_row_opt::<CollectionUuid>(row)?;

        debug!({collection_name = collection.name, collection_uuid = data}, "added collection");

        Ok(data)
    }

    #[instrument(skip_all)]
    async fn get_collection(
        &self,
        collection_uuid: CollectionUuid,
    ) -> anyhow::Result<Option<Collection>> {
        debug!(
            { collection_uuid = collection_uuid },
            "getting collection details"
        );

        let mut result = r"
            SELECT uid, gid, mtime, name, note, tags FROM collections WHERE collection_uuid = :collection_uuid"
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

        let data = from_row_opt::<(String, String, i64, String, String, String)>(row)?;

        debug!(
            { collection_uuid = collection_uuid },
            "found collection details"
        );

        Ok(Some(Collection {
            uid: data.0,
            gid: data.1,
            mtime: data.2,
            name: data.3,
            note: data.4,
            tags: unfold_set(&data.5),
        }))
    }

    async fn delete_collection(&self, collection_uuid: CollectionUuid) -> anyhow::Result<()> {
        debug!(
            { collection_uuid = collection_uuid },
            "deleting media from collection"
        );

        r"
            DELETE FROM collection_contents WHERE collection_uuid = :collection_uuid"
            .with(params! {
                "collection_uuid" => collection_uuid,
            })
            .run(self.pool.get_conn().await?)
            .await?;

        debug!({ collection_uuid = collection_uuid }, "deleting collection");

        r"
            DELETE FROM collections WHERE collection_uuid = :collection_uuid"
            .with(params! {
                "collection_uuid" => collection_uuid,
            })
            .run(self.pool.get_conn().await?)
            .await?;

        debug!({ collection_uuid = collection_uuid }, "deleted collection");

        Ok(())
    }

    #[instrument(skip_all)]
    async fn update_collection(
        &self,
        collection_uuid: CollectionUuid,
        update: CollectionUpdate,
    ) -> anyhow::Result<()> {
        debug!({ collection_uuid = collection_uuid }, "updating collection");

        if let Some(val) = update.name {
            r"
            UPDATE collections SET name = :name WHERE collection_uuid = :collection_uuid"
                .with(params! {
                    "name" => val.clone(),
                    "collection_uuid" => collection_uuid,
                })
                .run(self.pool.get_conn().await?)
                .await?;
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
        }

        r"
        UPDATE collections SET mtime = :mtime WHERE collection_uuid = :collection_uuid"
            .with(params! {
                "mtime" => Local::now().timestamp(),
                "collection_uuid" => collection_uuid,
            })
            .run(self.pool.get_conn().await?)
            .await?;

        debug!({ collection_uuid = collection_uuid }, "updated collection");

        Ok(())
    }

    #[instrument(skip_all)]
    async fn add_media_to_collection(
        &self,
        media_uuid: MediaUuid,
        collection_uuid: CollectionUuid,
    ) -> anyhow::Result<()> {
        debug!({media_uuid = media_uuid, collection_uuid = collection_uuid}, "adding media to collection");

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

        debug!({media_uuid = media_uuid, collection_uuid = collection_uuid}, "added media to collection");

        Ok(())
    }

    #[instrument(skip_all)]
    async fn rm_media_from_collection(
        &self,
        media_uuid: MediaUuid,
        collection_uuid: CollectionUuid,
    ) -> anyhow::Result<()> {
        debug!({media_uuid = media_uuid, collection_uuid = collection_uuid}, "removing media to collection");

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

        debug!({media_uuid = media_uuid, collection_uuid = collection_uuid}, "removed media from collection");

        Ok(())
    }

    #[instrument(skip_all)]
    async fn search_collections(
        &self,
        gid: HashSet<String>,
        filter: SearchFilter,
    ) -> anyhow::Result<Vec<CollectionUuid>> {
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
            .map(|row| from_row_opt::<CollectionUuid>(row))
            .collect::<Result<Vec<_>, FromRowError>>()?;

        Ok(data)
    }

    #[instrument(skip_all)]
    async fn search_media_in_collection(
        &self,
        gid: HashSet<String>,
        collection_uuid: CollectionUuid,
        filter: SearchFilter,
    ) -> anyhow::Result<Vec<MediaUuid>> {
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
            .map(|row| from_row_opt::<MediaUuid>(row))
            .collect::<Result<Vec<_>, FromRowError>>()?;

        Ok(data)
    }

    // library queries
    #[instrument(skip_all)]
    async fn add_library(&self, library: Library) -> anyhow::Result<LibraryUuid> {
        // since we require that libraries have unique paths, it might seem like we want
        // to use that as the primary key.  but those strings might be arbitrarily complex,
        // so instead using an i64 as a handle is much simpler
        debug!({ library_path = library.path }, "adding library");

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

        debug!({library_path = library.path, library_uuid = data}, "adding library");

        Ok(data)
    }

    #[instrument(skip_all)]
    async fn get_library(&self, library_uuid: LibraryUuid) -> anyhow::Result<Option<Library>> {
        debug!({ library_uuid = library_uuid }, "getting library details");

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

        debug!({ library_uuid = library_uuid }, "found library details");

        Ok(Some(Library {
            path: data.0,
            uid: data.1,
            gid: data.2,
            mtime: data.3,
            count: data.4,
        }))
    }

    #[instrument(skip_all)]
    async fn update_library(
        &self,
        library_uuid: LibraryUuid,
        update: LibraryUpdate,
    ) -> anyhow::Result<()> {
        debug!({ library_uuid = library_uuid }, "updating library");

        if let Some(val) = update.count {
            r"
            UPDATE libraries SET mtime = :mtime, count = :count WHERE library_uuid = :library_uuid"
                .with(params! {
                    "mtime" => Local::now().timestamp(),
                    "count" => val.clone(),
                    "library_uuid" => library_uuid,
                })
                .run(self.pool.get_conn().await?)
                .await?;
        }

        debug!({ library_uuid = library_uuid }, "updated library");

        Ok(())
    }

    #[instrument(skip_all)]
    async fn search_libraries(
        &self,
        gid: HashSet<String>,
        filter: String,
    ) -> anyhow::Result<Vec<LibraryUuid>> {
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
            .map(|row| from_row_opt::<LibraryUuid>(row))
            .collect::<Result<Vec<_>, FromRowError>>()?;

        Ok(data)
    }

    #[instrument(skip_all)]
    async fn search_media_in_library(
        &self,
        gid: HashSet<String>,
        library_uuid: LibraryUuid,
        hidden: bool,
        filter: SearchFilter,
    ) -> anyhow::Result<Vec<MediaUuid>> {
        let (sql, filter) = filter.format_mariadb("media.path, media.date, media.note, media.tags");

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
                media.hidden = :hidden"
            .to_owned();

        query.push_str(&sql);

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
            .map(|row| from_row_opt::<MediaUuid>(row))
            .collect::<Result<Vec<_>, FromRowError>>()?;

        Ok(data)
    }

    // comment queries
    #[instrument(skip_all)]
    async fn add_comment(&self, comment: Comment) -> anyhow::Result<CommentUuid> {
        debug!({ media_uuid = comment.media_uuid }, "adding comment");

        let mut result = r"
            INSERT INTO comments (comment_uuid, media_uuid, mtime, uid, text)
            VALUES (UUID_SHORT(), :media_uuid, :mtime, :uid, :text)
            RETURNING comment_uuid"
            .with(params! {
                "media_uuid" => comment.media_uuid,
                "mtime" => comment.mtime,
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

        debug!({media_uuid = comment.media_uuid, comment_uuid = data}, "added comment");

        Ok(data)
    }

    #[instrument(skip_all)]
    async fn get_comment(&self, comment_uuid: CommentUuid) -> anyhow::Result<Option<Comment>> {
        debug!({ comment_uuid = comment_uuid }, "getting comment details");

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

        debug!({ comment_uuid = comment_uuid }, "found comment details");

        Ok(Some(Comment {
            media_uuid: data.0,
            mtime: data.1,
            uid: data.2,
            text: data.3,
        }))
    }

    #[instrument(skip_all)]
    async fn delete_comment(&self, comment_uuid: CommentUuid) -> anyhow::Result<()> {
        debug!({ comment_uuid = comment_uuid }, "deleting comment");

        r"
        DELETE FROM comments WHERE (comment_uuid = :comment_uuid)"
            .with(params! {
                "comment_uuid" => comment_uuid,
            })
            .run(self.pool.get_conn().await?)
            .await?;

        debug!({ comment_uuid = comment_uuid }, "deleted comment");

        Ok(())
    }

    #[instrument(skip_all)]
    async fn update_comment(
        &self,
        comment_uuid: CommentUuid,
        text: Option<String>,
    ) -> anyhow::Result<()> {
        debug!({ comment_uuid = comment_uuid }, "updating comment");

        if let Some(val) = text {
            r"
            UPDATE comments SET text = :text WHERE comment_uuid = :comment_uuid"
                .with(params! {
                    "text" => val.clone(),
                    "comment_uuid" => comment_uuid,
                })
                .run(self.pool.get_conn().await?)
                .await?;
        }

        debug!({ comment_uuid = comment_uuid }, "updated comment");

        Ok(())
    }
}
