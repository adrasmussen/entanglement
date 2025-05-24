use std::{
    collections::HashSet,
    fs::Metadata,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    },
    time::Duration,
};

use anyhow::Result;
use chrono::Local;
use dashmap::DashSet;
use tokio::{
    fs::{canonicalize, create_dir_all, metadata, remove_file, symlink, try_exists},
    time::timeout,
};
use tracing::{Level, debug, instrument, span, warn};
use walkdir::DirEntry;

use crate::{
    db::msg::DbMsg,
    fs::{media_link_path, media_thumbnail_path},
    service::EsmSender,
};
use api::{
    library::LibraryUuid,
    media::{Media, MediaMetadata, MediaUuid},
};
use common::{
    config::ESConfig,
    media::{
        MediaData, content_hash, create_thumbnail, image::process_image, video::process_video,
    },
};

// scan_utils
//
// this module is mostly tooling for running library scans, including convenience functions,
// a global scan context, and a per-file context that automatically drops any filesystem
// resources needed to process the incoming media.
const KNOWN_EXTENSIONS: &[&str] = &["jpg", "png", "tiff", "mp4"];

pub async fn get_path_and_metadata(
    entry: walkdir::Result<DirEntry>,
) -> Result<(PathBuf, Metadata)> {
    let entry = entry?;
    let path = canonicalize(entry.path()).await?;
    let metadata = metadata(&path).await?;
    Ok((path, metadata))
}

async fn path_exists_in_database(
    context: Arc<ScanContext>,
    pathstr: &str,
) -> Result<Option<MediaUuid>> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    context
        .db_svc_sender
        .send(
            DbMsg::GetMediaUuidByPath {
                resp: tx,
                path: pathstr.to_owned(),
            }
            .into(),
        )
        .await?;

    let result = rx.await??;

    Ok(result)
}

async fn hash_exists_in_database(
    context: Arc<ScanContext>,
    hash: &str,
) -> Result<Option<MediaUuid>> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    context
        .db_svc_sender
        .send(
            DbMsg::GetMediaUuidByCHash {
                resp: tx,
                library_uuid: context.library_uuid,
                chash: hash.to_owned(),
            }
            .into(),
        )
        .await?;

    let result = rx.await??;

    Ok(result)
}

// per-scan global context
//
// this is the configuration struct for the scan tasks, as well as carrying global state
// used to report the results at the end
#[derive(Debug)]
pub struct ScanContext {
    pub config: Arc<ESConfig>,
    pub library_uuid: LibraryUuid,
    pub db_svc_sender: EsmSender,
    pub file_count: AtomicI64,
    pub warnings: AtomicI64,
    pub chashes: DashSet<String>,
    pub scratch_base: PathBuf,
}

impl Drop for ScanContext {
    fn drop(&mut self) {
        if std::fs::remove_dir_all(&self.scratch_base).is_err() {
            let _ = span!(Level::INFO, "scan_context_drop").entered();
            warn!("failed to clean up scratch base directory");
        }
    }
}

// per-file scan context
//
// this struct holds all of the relevant metadata for scanning an individual file, as well
// as any filesystem resources needed to process that file.  currently, that consists of a
// scratch dir used by the video processor to place temporary files, but could be extended.
//
// it seems vaguely wrong to effectively use a try/catch pattern on register(), given that
// we have to use a helper future to match the Result and emit a warn event to the logger.
// on the flip side, this lets the caller deal with any and all errors in whatever way
// makes the most sense to them.
//
// to simplify the cleanup in those cases, we modify Drop to remove the scratch directory,
// even though the caller ideally will wipe out the parent directory when the task ends.
#[derive(Clone, Debug)]
pub struct ScanFile {
    context: Arc<ScanContext>,
    path: PathBuf,
    ext: String,
    pathstr: String,
    metadata: Metadata,
    hash: String,
    scratch_dir: PathBuf,
}

impl Drop for ScanFile {
    fn drop(&mut self) {
        if std::fs::remove_dir_all(&self.scratch_dir).is_err() {
            let _ = span!(Level::INFO, "scan_file_drop", path = ?self.path).entered();
            warn!("failed to clean up scratch directory");
            self.context.warnings.fetch_add(1, Ordering::Relaxed);
        }
    }
}

impl ScanFile {
    // since this struct is designed for the scan task specifically, we can return Ok(None) if we
    // conclude that there is no actual work to be done for that file
    pub async fn new(
        context: Arc<ScanContext>,
        path: PathBuf,
        metadata: Metadata,
    ) -> Result<Option<Self>> {
        let context = context.clone();

        if !metadata.is_file() {
            return Err(anyhow::Error::msg(format!("{path:?} is not a file")));
        }

        let pathstr = path
            .to_str()
            .ok_or_else(|| anyhow::Error::msg("failed to convert path to str"))?
            .to_owned();

        // in lieu of some more complicated introspection, we rely on the file extention being
        // a (mostly) correct representation of the file's contents.  both the image and video
        // collectors are somewhat flexible on their inputs.
        let ext = path
            .extension()
            .and_then(|f| f.to_str())
            .map(|s| s.to_lowercase())
            .ok_or_else(|| anyhow::Error::msg("failed to extract file extention"))?;

        if !KNOWN_EXTENSIONS.contains(&ext.as_str()) {
            debug!({ path = pathstr }, "unknown file extension");
            return Ok(None);
        }

        // check if the file exists in the database first, and if so, early return before we do
        // any of the actual computation (hashes, thumbnails, etc)
        //
        // TODO -- use a bigger enum to encode "skipped" so that the file count is accurate
        if let Some(media_uuid) = path_exists_in_database(context.clone(), &pathstr).await? {
            debug!(
                { media_uuid = media_uuid },
                "media already exists in database"
            );
            return Ok(None);
        }

        let hash = content_hash(&path).await?;

        let scratch_dir = context.scratch_base.join(&hash);

        create_dir_all(&scratch_dir).await?;

        Ok(Some(ScanFile {
            context,
            path,
            ext,
            pathstr,
            metadata,
            hash,
            scratch_dir,
        }))
    }

    // media registration
    //
    // to prevent the server from just hanging, we use this timeout, although the caller
    // may have additional rules that it follows for the returned Future.  also note that
    // in the current implementation, the caller handles instrumentation; this keeps the
    // scan and any of its Errors in the same tracing span.
    pub async fn timed_register(&self) -> Result<()> {
        //let timeout = self.context.config.scan_timeout;
        let register_timeout = Duration::from_secs(300);

        timeout(register_timeout, self.register()).await?
    }

    #[instrument(skip_all)]
    async fn register(&self) -> Result<()> {
        debug!("processing media");

        // concurrent registration check
        //
        // for the deduplicator to run in a sane way, we need to first ensure that only
        // one instance of a particular content hash is (possibly) being added to the
        // database at a time.
        if !self.context.chashes.insert(self.hash.clone()) {
            debug!("duplicate media found (concurrent check)");
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
        if let Some(media_uuid) = hash_exists_in_database(self.context.clone(), &self.hash).await? {
            let media = {
                let (tx, rx) = tokio::sync::oneshot::channel();

                self.context
                    .db_svc_sender
                    .send(
                        DbMsg::GetMedia {
                            resp: tx,
                            media_uuid,
                        }
                        .into(),
                    )
                    .await?;

                rx.await??
                    .ok_or_else(|| {
                        anyhow::Error::msg(
                            "internal error: failed to get_media after locating hash",
                        )
                    })?
                    .0
            };

            if try_exists(&media.path).await? {
                debug!("duplicate media found (move check)");
            } else {
                let (tx, rx) = tokio::sync::oneshot::channel();
                self.context
                    .db_svc_sender
                    .send(
                        DbMsg::ReplaceMediaPath {
                            resp: tx,
                            media_uuid,
                            path: self.pathstr.to_owned(),
                        }
                        .into(),
                    )
                    .await?;

                rx.await??;

                self.install(media_uuid, media.metadata).await?;
            }

            return Ok(());
        }

        // media processing
        //
        // in lieu of some more complicated introspection, we rely on the file extention being
        // a (mostly) correct representation of the file's contents.  both the image and video
        // collectors are somewhat flexible on their inputs.
        //
        // this list is not even close to exhaustive, and future improvements are expected.
        //
        // note that the extensions are also used by the http service to guess the mime type of
        // the media files; see http/stream.rs for details.
        let media_data: MediaData = match self.ext.as_str() {
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
            .send(DbMsg::AddMedia { resp: tx, media }.into())
            .await?;

        let media_uuid = rx.await??;

        self.install(media_uuid, media_data.metadata).await?;

        debug!("finished processing media");
        Ok(())
    }

    // media "installation"
    //
    // to actually access the media, we use symlinks.  this allows the http server to function like
    // an object store without needing to reorganize the filesystem. see also http/stream.rs.
    //
    // currently this step consists of two steps, but in principle any postprocessing needed to use
    // media should go here as well.  it may be that we split out this function if its internals are
    // useful for the dedup or cleaning tasks.
    #[instrument(skip(self, media_metadata))]
    async fn install(&self, media_uuid: MediaUuid, media_metadata: MediaMetadata) -> Result<()> {
        debug!("creating symlinks and thumbnails");

        // symlink
        let symlink_path = media_link_path(self.context.config.clone(), media_uuid);

        let _ = remove_file(&symlink_path).await;

        symlink(&self.path, &symlink_path).await?;

        // thumbnail
        let thumbnail_path = media_thumbnail_path(self.context.config.clone(), media_uuid);

        // if the thumbnail already exists (because we are re-running create_links() due to moved media), then
        // don't recreate the thmbnail
        if try_exists(&thumbnail_path).await? {
            return Ok(());
        }

        create_thumbnail(
            &self.path,
            &thumbnail_path,
            &self.scratch_dir,
            &media_metadata,
        )
        .await?;

        Ok(())
    }
}
