use std::{collections::HashMap, path::PathBuf, sync::Arc};

use anyhow::Result;
use api::{
    UuidSource, collection::CollectionUuid, library::LibraryUuid,
    media::Media,
};
use rocksdb::DB;
use serde::{Deserialize, Serialize};

use common::{
    config::ESConfig,
    db::{DbBackend, MariaDBBackend},
};

const MEDIA_PREFIX: &str = "media_";
const COLLECTION_PREFIX: &str = "collection_";
const COMMENT_PREFIX: &str = "comment_";
const LIBRARY_PREFIX: &str = "library_";

#[derive(Serialize, Deserialize)]
struct MediaRecord {
    media: Media,
    collections: Vec<CollectionUuid>,
}

pub async fn dump(config: Arc<ESConfig>, filename: PathBuf) -> Result<()> {
    let db: Box<dyn DbBackend> = match config.db_backend {
        common::config::DbBackend::MariaDB => Box::new(MariaDBBackend::new(config).await?),
        _ => todo!(),
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
            format!("{LIBRARY_PREFIX}{library_uuid}"),
            serde_json::to_vec(&library)?,
        )?;
    }

    // media
    let media_uuids = db.get_media_uuids().await?;

    for media_uuid in media_uuids.into_iter() {
        let (media, collections, _comments) = db
            .get_media(media_uuid)
            .await?
            .ok_or_else(|| anyhow::Error::msg("missing media"))?;

        let record = serde_json::to_vec(&MediaRecord { media, collections })?;

        rdb.put(format!("{MEDIA_PREFIX}{media_uuid}"), record)?;
    }

    // comments
    let comment_uuids = db.get_comment_uuids().await?;

    for comment_uuid in comment_uuids.into_iter() {
        let comment = db
            .get_comment(comment_uuid)
            .await?
            .ok_or_else(|| anyhow::Error::msg("missing comment"))?;

        rdb.put(
            format!("{COMMENT_PREFIX}{comment_uuid}"),
            serde_json::to_vec(&comment)?,
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
            format!("{COLLECTION_PREFIX}{collection_uuid}"),
            serde_json::to_vec(&collection)?,
        )?;
    }

    Ok(())
}

struct UndumpParser;

impl UuidSource for UndumpParser {}

pub async fn undump(config: Arc<ESConfig>, filename: PathBuf) -> Result<()> {
    let parser = UndumpParser;

    let db: Box<dyn DbBackend> = match config.db_backend {
        common::config::DbBackend::MariaDB => Box::new(MariaDBBackend::new(config).await?),
        _ => todo!(),
    };

    let mut opts = rocksdb::Options::default();
    opts.create_if_missing(true);

    let rdb = DB::open(&opts, &filename)?;

    // libraries
    let mut library_map = HashMap::new();

    let library_iter = rdb.prefix_iterator(LIBRARY_PREFIX);

    for item in library_iter {
        let (key, value) = item?;

        let old_uuid = LibraryUuid::try_parse(&parser, &String::from_utf8(key.to_vec())?)?;

        let library_uuid = db.add_library(serde_json::from_slice(&value)?).await?;

        library_map
            .insert(old_uuid, library_uuid)
            .ok_or_else(|| anyhow::Error::msg("duplicate library_uuid"))?;
    }

    // media
    let mut content_map = HashMap::new();

    let media_iter = rdb.prefix_iterator(MEDIA_PREFIX);

    for item in media_iter {
        let (_key, value) = item?;

        let mut record: MediaRecord = serde_json::from_slice(&value)?;

        record.media.library_uuid = *library_map
            .get(&record.media.library_uuid)
            .ok_or_else(|| anyhow::Error::msg("missing library_uuid"))?;

        let media_uuid = db.add_media(record.media).await?;

        content_map
            .insert(media_uuid, record.collections)
            .ok_or_else(|| anyhow::Error::msg("duplicate media_uuid"))?;
    }

    // comments
    let comment_iter = rdb.prefix_iterator(COMMENT_PREFIX);

    for item in comment_iter {
        let (_key, value) = item?;

        db.add_comment(serde_json::from_slice(&value)?).await?;
    }

    // collections
    let mut collection_map = HashMap::new();

    let collection_iter = rdb.prefix_iterator(COLLECTION_PREFIX);

    for item in collection_iter {
        let (key, value) = item?;

        let old_uuid = CollectionUuid::try_parse(&parser, &String::from_utf8(key.to_vec())?)?;

        let collection_uuid = db.add_collection(serde_json::from_slice(&value)?).await?;

        collection_map
            .insert(old_uuid, collection_uuid)
            .ok_or_else(|| anyhow::Error::msg("duplicate collection_uuid"))?;
    }

    // restore media back to their original collections
    for (media_uuid, old_collections) in content_map.iter() {
        let new_collections = old_collections
            .iter()
            .map(|c| {
                collection_map
                    .get(c)
                    .ok_or_else(|| anyhow::Error::msg("missing collection uuid"))
                    .cloned()
            })
            .collect::<Result<Vec<CollectionUuid>>>()?;

        for collection_uuid in new_collections.iter() {
            db.add_media_to_collection(*media_uuid, *collection_uuid).await?;
        }
    }

    Ok((

    ))
}
