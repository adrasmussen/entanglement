use std::{path::PathBuf, sync::Arc};

use anyhow::Result;
use api::{collection::CollectionUuid, comment::CommentUuid, media::Media};
use rocksdb::DB;
use serde::{Deserialize, Serialize};

use common::{
    config::ESConfig,
    db::{DbBackend, MariaDBBackend},
};

#[derive(Serialize, Deserialize)]
struct MediaRecord {
    media: Media,
    collections: Vec<CollectionUuid>,
    comments: Vec<CommentUuid>,
}

pub async fn dump(config: Arc<ESConfig>, filename: PathBuf) -> Result<()> {
    let db: Box<dyn DbBackend> = match config.db_backend {
        common::config::DbBackend::MariaDB => Box::new(MariaDBBackend::new(config)?),
    };

    let mut opts = rocksdb::Options::default();
    opts.create_if_missing(true);

    let rdb = DB::open(&opts, &filename)?;

    // libraries
    let library_uuids = db.get_library_uuids().await?;

    for library_uuid in library_uuids.into_iter() {
        let library = db
            .get_library(library_uuid)
            .await?
            .ok_or_else(|| anyhow::Error::msg("missing library"))?;

        rdb.put(
            format!("library_{library_uuid}"),
            serde_json::to_vec(&library)?,
        )?;
    }

    // collections
    let collection_uuids = db.get_collection_uuids().await?;

    for collection_uuid in collection_uuids.into_iter() {
        let collection = db
            .get_collection(collection_uuid)
            .await?
            .ok_or_else(|| anyhow::Error::msg("missing collection"))?;

        rdb.put(
            format!("collection_{collection_uuid}"),
            serde_json::to_vec(&collection)?,
        )?;
    }

    // comments
    let comment_uuids = db.get_comment_uuids().await?;

    for comment_uuid in comment_uuids.into_iter() {
        let comment = db
            .get_comment(comment_uuid)
            .await?
            .ok_or_else(|| anyhow::Error::msg("missing comment"))?;

        rdb.put(
            format!("comment_{comment_uuid}"),
            serde_json::to_vec(&comment)?,
        )?;
    }

    // media

    let media_uuids = db.get_media_uuids().await?;

    for media_uuid in media_uuids.into_iter() {
        let (media, collections, comments) = db
            .get_media(media_uuid)
            .await?
            .ok_or_else(|| anyhow::Error::msg("missing media"))?;

        let key = format!("media_{}", media.chash);

        let record = serde_json::to_vec(&MediaRecord {
            media,
            collections,
            comments,
        })?;

        rdb.put(key, record)?;
    }

    Ok(())
}

pub async fn undump(config: Arc<ESConfig>, filename: PathBuf) -> Result<()> {
    todo!()
}
