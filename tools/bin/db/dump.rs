use std::{collections::HashMap, path::PathBuf, sync::Arc};

use anyhow::Result;
use api::{
    UuidSource,
    collection::CollectionUuid,
    comment::Comment,
    library::LibraryUuid,
    media::{Media, MediaUuid},
};
use rocksdb::{DB, SliceTransform};
use serde::{Deserialize, Serialize};

use common::{
    config::ESConfig,
    db::{DbBackend, MariaDBBackend, PostgresBackend},
};
use uuid::Uuid;

const MEDIA_PREFIX: u8 = 0x01;
const COLLECTION_PREFIX: u8 = 0x02;
const COMMENT_PREFIX: u8 = 0x03;
const LIBRARY_PREFIX: u8 = 0x04;

fn make_key(prefix: u8, uuid: Uuid) -> Vec<u8> {
    let mut key = vec![prefix];
    key.extend_from_slice(uuid.as_bytes());
    key
}

fn unmake_key(key: &[u8]) -> Result<Uuid> {
    let (_prefix, uuid) = key.split_at_checked(1).ok_or_else(|| anyhow::Error::msg("invalid key"))?;

    Ok(Uuid::from_bytes(uuid.try_into()?))
}

#[derive(Serialize, Deserialize)]
struct MediaRecord {
    media: Media,
    collections: Vec<CollectionUuid>,
}

pub async fn dump(config: Arc<ESConfig>, filename: PathBuf) -> Result<()> {
    let db: Box<dyn DbBackend> = match config.db_backend {
        common::config::DbBackend::MariaDB => Box::new(MariaDBBackend::new(config).await?),
        common::config::DbBackend::Postgres => Box::new(PostgresBackend::new(config).await?),
    };

    let mut opts = rocksdb::Options::default();
    opts.create_if_missing(true);

    let rdb = DB::open(&opts, &filename)?;

    // libraries
    println!("fetching libraries");
    let library_iter = db.get_library_uuids().await?.into_iter();

    for (i, library_uuid) in library_iter.enumerate() {
        print!("\r  libraries: {}", i + 1);

        let library = db
            .get_library(library_uuid)
            .await?
            .ok_or_else(|| anyhow::Error::msg("missing library"))?;

        rdb.put(
            make_key(LIBRARY_PREFIX, library_uuid.value()),
            serde_json::to_vec(&library)?,
        )?;
    }
    println!("  complete");

    // media
    println!("fetching media");
    let media_iter = db.get_media_uuids().await?.into_iter();

    for (i, media_uuid) in media_iter.enumerate() {
        print!("\r  media: {}", i + 1);

        let (media, collections, _comments) = db
            .get_media(media_uuid)
            .await?
            .ok_or_else(|| anyhow::Error::msg("missing media"))?;

        let record = serde_json::to_vec(&MediaRecord { media, collections })?;

        rdb.put(make_key(MEDIA_PREFIX, media_uuid.value()), record)?;
    }
    println!("  complete");

    // comments
    println!("fetching comments");
    let comment_iter = db.get_comment_uuids().await?.into_iter();

    for (i, comment_uuid) in comment_iter.enumerate() {
        print!("\r  comments: {}", i + 1);

        let comment = db
            .get_comment(comment_uuid)
            .await?
            .ok_or_else(|| anyhow::Error::msg("missing comment"))?;

        rdb.put(
            make_key(COMMENT_PREFIX, comment_uuid.value()),
            serde_json::to_vec(&comment)?,
        )?;
    }
    println!("  complete");

    // collections
    println!("fetching collections");
    let collection_iter = db.get_collection_uuids().await?.into_iter();

    for (i, collection_uuid) in collection_iter.enumerate() {
        print!("\r  collections: {}", i + 1);

        let collection = db
            .get_collection(collection_uuid)
            .await?
            .ok_or_else(|| anyhow::Error::msg("missing collection"))?;

        rdb.put(
            make_key(COLLECTION_PREFIX, collection_uuid.value()),
            serde_json::to_vec(&collection)?,
        )?;
    }
    println!("  complete");

    println!("database dump complete");

    Ok(())
}

struct UndumpParser;

impl UuidSource for UndumpParser {}

pub async fn undump(config: Arc<ESConfig>, filename: PathBuf) -> Result<()> {
    let parser = UndumpParser;

    let db: Box<dyn DbBackend> = match config.db_backend {
        common::config::DbBackend::MariaDB => Box::new(MariaDBBackend::new(config).await?),
        common::config::DbBackend::Postgres => Box::new(PostgresBackend::new(config).await?),
    };

    let mut opts = rocksdb::Options::default();
    opts.create_if_missing(true);
    opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(1));

    let rdb = DB::open(&opts, &filename)?;

    // libraries
    println!("creating libraries");
    let mut library_map = HashMap::new();

    let library_iter = rdb.prefix_iterator([LIBRARY_PREFIX]);

    for (i, item) in library_iter.enumerate() {
        print!("\r  libraries: {}", i + 1);

        let (key, value) = item?;

        let old_uuid = LibraryUuid::from_value(&parser, unmake_key(&key)?);

        let library_uuid = db.add_library(serde_json::from_slice(&value)?).await?;

        if library_map.insert(old_uuid, library_uuid).is_some() {
            return Err(anyhow::Error::msg("duplicate library_uuid"));
        };
    }
    println!("  complete");

    // media
    println!("creating media");
    let mut media_map = HashMap::new();
    let mut content_map = HashMap::new();

    let media_iter = rdb.prefix_iterator([MEDIA_PREFIX]);

    for (i, item) in media_iter.enumerate() {
        print!("\r  media: {}", i + 1);

        let (key, value) = item?;

        let mut record: MediaRecord = serde_json::from_slice(&value)?;

        // translate library uuid
        record.media.library_uuid = *library_map
            .get(&record.media.library_uuid)
            .ok_or_else(|| anyhow::Error::msg("missing library_uuid"))?;

        let old_uuid = MediaUuid::from_value(&parser, unmake_key(&key)?);

        let media_uuid = db.add_media(record.media).await?;

        if media_map.insert(old_uuid, media_uuid).is_some() {
            return Err(anyhow::Error::msg("duplicate media_uuid"));
        };

        if content_map.insert(media_uuid, record.collections).is_some() {
            return Err(anyhow::Error::msg("duplicate media_uuid"));
        };
    }
    println!("  complete");

    // comments
    println!("creating comments");
    let comment_iter = rdb.prefix_iterator([COMMENT_PREFIX]);

    for (i, item) in comment_iter.enumerate() {
        print!("\r  comments: {}", i + 1);

        let (_key, value) = item?;

        let mut comment: Comment = serde_json::from_slice(&value)?;

        // translate media uuid
        comment.media_uuid = *media_map
            .get(&comment.media_uuid)
            .ok_or_else(|| anyhow::Error::msg("missing media_uuid"))?;

        db.add_comment(serde_json::from_slice(&value)?).await?;
    }
    println!("  complete");

    // collections
    println!("creating collections");
    let mut collection_map = HashMap::new();

    let collection_iter = rdb.prefix_iterator([COLLECTION_PREFIX]);

    for (i, item) in collection_iter.enumerate() {
        print!("\r  collections: {}", i + 1);

        let (key, value) = item?;

        let old_uuid = CollectionUuid::from_value(&parser, unmake_key(&key)?);

        let collection_uuid = db.add_collection(serde_json::from_slice(&value)?).await?;

        if collection_map.insert(old_uuid, collection_uuid).is_some() {
            return Err(anyhow::Error::msg("duplicate collection_uuid"));
        };
    }
    println!("  complete");

    // restore media back to their original collections
    println!("putting media into collections");

    let content_iter = content_map.iter();

    for (i, item) in content_iter.enumerate() {
        print!("\r  links: {}", i + 1);

        let (media_uuid, old_collections) = item;

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
            db.add_media_to_collection(*media_uuid, *collection_uuid)
                .await?;
        }
    }
    println!("  complete");

    println!("database upload complete");

    Ok(())
}
