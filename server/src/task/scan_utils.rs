use std::{
    collections::HashSet,
    fs::Metadata,
    path::PathBuf,
    sync::{atomic::AtomicI64, Arc},
    time::Duration,
};

use anyhow::Result;
use chrono::Local;
use dashmap::DashSet;
use hex::encode;
use sha2::{Digest, Sha512};
use tokio::{
    fs::{canonicalize, create_dir_all, metadata, symlink, try_exists, File},
    io::{AsyncReadExt, BufReader},
    time::timeout,
};
use tracing::{debug, error, info, instrument, span, warn, Instrument, Level};
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

struct ScanContext {
    config: Arc<ESConfig>,
    library_uuid: LibraryUuid,
    db_svc_sender: ESMSender,
    file_count: Arc<AtomicI64>,
    warnings: Arc<AtomicI64>,
    chashes: DashSet<String>,
    scratch_base: PathBuf,
}

pub struct FileScan {
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
    async fn new(
        context: Arc<ScanContext>,
        entry: walkdir::Result<DirEntry>,
    ) -> Result<Option<Self>> {
        let context = context.clone();

        let (path, metadata) = get_path_and_metadata(entry).await?;

        let pathstr = path
            .to_str()
            .ok_or_else(|| anyhow::Error::msg("failed to convert path to str"))?
            .to_owned();

        // since this struct represents a media file that we want to reason about, we immediately skip
        // any file whose path already exists in the database... before we do any expensive computation
        if FileScan::path_exists_in_database(context.clone(), &pathstr).await? {
            return Ok(None);
        }

        let hash = content_hash(&path).await?;

        let scratch_dir = context.scratch_base.join(&hash);

        create_dir_all(&scratch_dir).await?;

        Ok(Some(FileScan {
            context,
            path,
            pathstr,
            metadata,
            hash,
            scratch_dir,
        }))
    }

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

    async fn timed_register(&self) -> Result<()> {
        //let timeout = self.context.config.scan_timeout;
        let register_timeout = Duration::from_secs(300);

        timeout(
            register_timeout,
            self.register()
                .instrument(span!(Level::INFO, "register_media", path = self.pathstr)),
        )
        .await?
    }

    // media registration
    async fn register(&self) -> Result<()> {
        debug!("processing media");

        // concurrent registration check
        //
        // for the deduplicator to run in a sane way, we need to first ensure that only
        // one instance of a particular content hash is (possibly) being added to the
        // database at a time.
        if self.context.chashes.insert(self.hash.clone()) {
            debug!("duplicate media found");
            return Ok(());
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
            let media = {
                let (tx, rx) = tokio::sync::oneshot::channel();
                self.context
                    .db_svc_sender
                    .send(
                        DbMsg::GetMedia {
                            resp: tx,
                            media_uuid: media_uuid,
                        }
                        .into(),
                    )
                    .await?;
                rx.await??
                    .ok_or_else(|| {
                        anyhow::Error::msg(
                            "internal error: failed to find media after locating hash",
                        )
                    })?
                    .0
            };

            if !try_exists(&media.path).await? {
                let (tx, rx) = tokio::sync::oneshot::channel();
                self.context
                    .db_svc_sender
                    .send(
                        DbMsg::ReplaceMediaPath {
                            resp: tx,
                            media_uuid: media_uuid,
                            path: self.pathstr.to_owned(),
                        }
                        .into(),
                    )
                    .await?;

                rx.await??;

                self.create_links(media_uuid, media.metadata).await?;
                return Ok(());
            }
        } else {
            debug!("duplicate media found");
            return Ok(());
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

        self.create_links(media_uuid, media_data.metadata).await?;

        debug!("finished processing media");
        Ok(())
    }

    #[instrument(skip(self, media_metadata))]
    async fn create_links(
        &self,
        media_uuid: MediaUuid,
        media_metadata: MediaMetadata,
    ) -> Result<()> {
        // once the media is successfully registered, we create the symlink and thumbnails
        //
        // cleaner tasks will also re-create these if something goes wrong
        debug!("creating symlinks and thumbnails");

        symlink(
            self.path.clone(),
            media_original_path(self.context.config.clone(), media_uuid),
        )
        .await?;

        let media_thumbnail_path = media_thumbnail_path(self.context.config.clone(), media_uuid);

        match media_metadata {
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

        Ok(())
    }
}
