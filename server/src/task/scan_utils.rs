use std::{
    collections::HashSet,
    fs::Metadata,
    path::PathBuf,
    sync::{atomic::AtomicI64, Arc},
};

use anyhow::Result;
use chrono::Local;
use dashmap::DashSet;
use hex::encode;
use sha2::{Digest, Sha512};
use tokio::{
    fs::{canonicalize, metadata, symlink, File},
    io::{AsyncReadExt, BufReader},
};
use tracing::{debug, error, info, span, warn, Instrument, Level};
use walkdir::DirEntry;

use crate::{
    db::msg::DbMsg,
    fs::{media_original_path, media_thumbnail_path},
    service::ESMSender,
};
use api::{
    library::LibraryUuid,
    media::{Media, MediaMetadata, MediaUuid},
};
use common::{
    config::ESConfig,
    media::{
        image::{create_image_thumbnail, process_image},
        video::{create_video_thumbnail, process_video},
        MediaData,
    },
};

// probably move this defintion to common::media
async fn get_path_and_metadata(entry: walkdir::Result<DirEntry>) -> Result<(PathBuf, Metadata)> {
    let entry = entry?;
    let path = canonicalize(entry.path()).await?;
    let metadata = metadata(&path).await?;
    Ok((path, metadata))
}

async fn content_hash(path: &PathBuf) -> Result<String> {
    let file = File::open(&path).await?;

    let mut hasher = Sha512::new();
    let mut buffer = [0; 8192];

    // TODO -- perf tuning
    let mut reader = BufReader::with_capacity(8182, file);

    while reader.read(&mut buffer).await? > 0 {
        hasher.update(buffer);
    }

    Ok(encode(hasher.finalize()))
}

// this can run on the outside, before anyone has
async fn path_exists_in_database(context: Arc<ScanContext>, pathstr: &String) -> Result<bool> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    context
        .db_svc_sender
        .send(
            DbMsg::GetMediaUuidByPath {
                resp: tx,
                path: pathstr.clone(),
            }
            .into(),
        )
        .await?;

    Ok(rx
        .await??
        .inspect(|_| debug!("media already exists in database"))
        .is_some())
}

struct ScanContext {
    config: Arc<ESConfig>,
    library_uuid: LibraryUuid,
    db_svc_sender: ESMSender,
    file_count: Arc<AtomicI64>,
    warnings: Arc<AtomicI64>,
    chashes: DashSet<String>,
    scratch_base: PathBuf,
}

struct FileScan {
    context: Arc<ScanContext>,
    path: PathBuf,
    pathstr: String,
    metadata: Metadata,
    hash: String,
    scratch_dir: PathBuf,
}

impl Drop for FileScan {
    fn drop(&mut self) {
        if std::fs::remove_dir_all(&self.scratch_dir).is_err() {
            let _ = span!(Level::INFO, "file_scan_drop", path = ?self.path).entered();
            warn!("failed to clean up scratch directory")
        }
    }
}

impl FileScan {
    async fn new(context: Arc<ScanContext>, entry: walkdir::Result<DirEntry>) -> Result<Option<Self>> {
        let context = context.clone();

        let (path, metadata) = get_path_and_metadata(entry).await?;

        let pathstr = path
            .to_str()
            .ok_or_else(|| anyhow::Error::msg("failed to convert path to str"))?
            .to_owned();

        // since this struct represents a media file that we want to reason about, we immediately skip
        // any file whose path already exists in the database... before we do any expensive computation
        if path_exists_in_database(context.clone(), &pathstr).await? {
            return Ok(None)
        }

        let hash = content_hash(&path).await?;

        let scratch_dir = context.scratch_base.join(&hash);

        Ok(Some(FileScan {
            context,
            path,
            pathstr,
            metadata,
            hash,
            scratch_dir,
        }))
    }

    async fn hash_exists_in_database(&self) -> Result<Option<MediaUuid>> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.context
            .db_svc_sender
            .send(
                DbMsg::GetMediaUuidByCHash {
                    resp: tx,
                    library_uuid: self.context.library_uuid,
                    chash: self.hash.clone(),
                }
                .into(),
            )
            .await?;

        let result = rx.await??;

        Ok(result)
    }


    // deduplication thoughts
    //
    // if each media_uuid has a list of paths, then the scanner can memoize any duplicates
    // during the scan and update the paths all at the end (which avoids row locks in the
    // the database).  DashMap<String, Option<Vec<String>> could work, where None indicates
    // that a thread is doing the hard work and Some(Vec) are any duplicates encountered.
    //
    // in this model, only the concurrent registration check needs to put paths into the
    // bucket -- media

    async fn register(&self) -> Result<()> {
        // concurrent registration check
        //
        // for the deduplicator to run in a sane way, we need to first ensure that only
        // one instance of a particular content hash is (possibly) being added to the
        // database at a time.
        if self.context.chashes.insert(self.hash.clone()) {
            return Err(anyhow::Error::msg("duplicate media found"));
        }

        // moved media check
        //
        // if the current path is not in the database (see new()) but the content hash
        // matches an existing database entry, we check to see if the old media is still
        // there.  if not, we update the database.
        //
        // in the event that there are several media files with the same hash, the prior
        // check will ensure that we only do this once.  however, there are no guarantees
        // about which file will be reached first.
        if let Some(media_uuid) = self.hash_exists_in_database().await? {
            // get_media(media_uuid)
            // if exists(PathBuf::from(media.path)) {
            // db.send(ReplaceMediaPath(media_uuid, self.pathstr))
            //
            // suggests creation of MediaThumbnail and MediaLink objects
            // }
            todo!()
        }

        // match the metadata collector via file extension
        let ext = self
            .path
            .extension()
            .map(|f| f.to_str())
            .flatten()
            .ok_or_else(|| anyhow::Error::msg("failed to extract file extention"))?;

        let media_data: MediaData = match ext {
            "jpg" | "png" | "tiff" => process_image(&self.path).await?,
            "mp4" => process_video(&self.path, &self.scratch_dir).await?,
            _ => return Err(anyhow::Error::msg("no metadata collector for extension")),
        };

        // once we have the metadata, we assemble the Media struct and send it to the database
        let media = Media {
            library_uuid: self.context.library_uuid,
            path: self.pathstr.clone(),
            size: self.metadata.len(),
            chash: self.hash.clone(),
            phash: media_data.hash,
            mtime: Local::now().timestamp(),
            hidden: false,
            date: media_data.date,
            note: "".to_owned(),
            tags: HashSet::new(),
            metadata: media_data.metadata.clone(),
        };

        let (tx, rx) = tokio::sync::oneshot::channel();

        self.context
            .db_svc_sender
            .send(
                DbMsg::AddMedia {
                    resp: tx,
                    media: media,
                }
                .into(),
            )
            .await?;

        let media_uuid = rx.await??;

        // once the media is successfully registered, we create the symlink and thumbnails
        //
        // cleaner tasks will also re-create these if something goes wrong
        symlink(
            self.path.clone(),
            media_original_path(self.context.config.clone(), media_uuid),
        )
        .await?;

        let media_thumbnail_path = media_thumbnail_path(self.context.config.clone(), media_uuid);

        match media_data.metadata {
            MediaMetadata::Image => {
                Box::pin(create_image_thumbnail(&self.path, &media_thumbnail_path)).await?
            }
            MediaMetadata::Video => {
                Box::pin(create_video_thumbnail(
                    &self.path,
                    &media_thumbnail_path,
                    &self.scratch_dir,
                ))
                .await?
            }
            _ => return Err(anyhow::Error::msg("no thumbnail method found")),
        };

        debug!("finished processing media");
        Ok(())
    }
}
