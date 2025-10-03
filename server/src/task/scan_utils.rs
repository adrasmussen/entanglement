use std::{
    collections::HashSet,
    fs::Metadata,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    },
    time::UNIX_EPOCH,
};

use anyhow::Result;
use dashmap::{DashMap, DashSet};
use tokio::fs::{canonicalize, create_dir_all, metadata, remove_file, symlink};
use tracing::{Level, debug, instrument, span, warn};
use walkdir::DirEntry;

use crate::{
    db::msg::DbMsg,
    fs::{media_link_path, media_thumbnail_path},
    service::EsmSender,
};
use api::{
    FOLDING_SEPARATOR,
    library::LibraryUuid,
    media::{Media, MediaMetadata, MediaUpdate, MediaUuid},
};
use common::{
    config::ESConfig,
    db::{MediaByCHash, MediaByPath},
    media::{
        MediaData, content_hash, create_thumbnail, image::process_image, video::process_video,
    },
};

// scan_utils
//
// this module is mostly tooling for running library scans, including convenience functions,
// a global scan context, and a per-file context that automatically drops any filesystem
// resources needed to process the incoming media.
#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum FileStatus {
    Register(ScanFile),
    Exists(KnownFile),
    Skip,
    Unknown,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct KnownFile {
    pub media_uuid: MediaUuid,
    pub path: String,
    pub hash: String,
    pub mtime: u64,
}

#[derive(Clone, Debug)]
enum MediaType {
    Image,
    Video,
}

pub async fn get_path_and_metadata(
    entry: walkdir::Result<DirEntry>,
) -> Result<(PathBuf, Metadata)> {
    let entry = entry?;
    let path = canonicalize(entry.path()).await?;
    let metadata = metadata(&path).await?;
    Ok((path, metadata))
}

pub async fn add_tag_to_media(
    db_svc_sender: EsmSender,
    media_uuid: MediaUuid,
    new_tag: String,
) -> Result<()> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    db_svc_sender
        .send(
            DbMsg::GetMedia {
                resp: tx,
                media_uuid,
            }
            .into(),
        )
        .await?;

    let (media, _, _) = rx
        .await??
        .ok_or_else(|| anyhow::Error::msg(format!("unknown media_uuid: {media_uuid}")))?;

    let (tx, rx) = tokio::sync::oneshot::channel();

    let mut tags = media.tags.clone();

    if new_tag.contains(FOLDING_SEPARATOR) {
        return Err(anyhow::Error::msg(
            "internal error: new tag contains folding seperator",
        ));
    }

    tags.insert(new_tag);

    db_svc_sender
        .send(
            DbMsg::UpdateMedia {
                resp: tx,
                media_uuid,
                update: MediaUpdate {
                    hidden: None,
                    date: None,
                    note: None,
                    tags: Some(tags),
                },
            }
            .into(),
        )
        .await?;

    rx.await?
}

// in lieu of some more complicated introspection, we rely on the file extention being
// a (mostly) correct representation of the file's contents.  both the image and video
// collectors are somewhat flexible on their inputs.
//
// this list is not even close to exhaustive, and future improvements are expected.
//
// note that the extensions are also used by the http service to guess the mime type of
// the media files; see http/stream.rs for details.
fn get_mtype(path: &Path) -> Result<MediaType> {
    let ext = path
        .extension()
        .and_then(|f| f.to_str())
        .map(|s| s.to_lowercase())
        .ok_or_else(|| anyhow::Error::msg("failed to extract file extention"))?;

    match ext.as_str() {
        "jpg" | "png" | "tiff" => Ok(MediaType::Image),
        "mp4" | "avi" => Ok(MediaType::Video),
        _ => Err(anyhow::Error::msg(format!("unknown media extention {ext}"))),
    }
}

async fn create_scratch_dir(context: Arc<ScanContext>, chash: &str) -> Result<PathBuf> {
    let scratch_dir = context.scratch_base.join(chash);

    create_dir_all(&scratch_dir).await?;

    Ok(scratch_dir)
}

// per-scan global context
//
// this is the configuration struct for the scan tasks, which carries global
// state for reporting as well as the deduplication logic
#[derive(Debug)]
pub struct ScanContext {
    pub config: Arc<ESConfig>,
    pub library_uuid: LibraryUuid,
    pub db_svc_sender: EsmSender,
    pub file_count: AtomicI64,
    pub warnings: AtomicI64,
    pub known_files: DashSet<KnownFile>,
    pub chashes: DashSet<String>,
    pub scratch_base: PathBuf,
}

impl Drop for ScanContext {
    fn drop(&mut self) {
        if std::fs::remove_dir_all(&self.scratch_base).is_err() {
            let _span = span!(Level::INFO, "scan_context_drop").entered();
            warn!("failed to clean up scratch base directory");
        }
    }
}

impl ScanContext {
    #[instrument(skip_all)]
    async fn get_media_by_path(&self, pathstr: &str) -> Result<Option<MediaByPath>> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.db_svc_sender
            .send(
                DbMsg::GetMediaUuidByPath {
                    resp: tx,
                    path: pathstr.to_owned(),
                }
                .into(),
            )
            .await?;

        rx.await?
    }

    #[instrument(skip_all)]
    async fn get_media_by_chash(&self, hash: &str) -> Result<Option<MediaByCHash>> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.db_svc_sender
            .send(
                DbMsg::GetMediaUuidByCHash {
                    resp: tx,
                    library_uuid: self.library_uuid,
                    chash: hash.to_owned(),
                }
                .into(),
            )
            .await?;

        rx.await?
    }

    #[instrument(skip_all)]
    pub fn collate_known_files(&self) -> Result<DashMap<MediaUuid, Vec<KnownFile>>> {
        debug!("collating known files");

        let known_files = DashMap::<MediaUuid, Vec<KnownFile>>::new();

        for item in self.known_files.iter() {
            if known_files.contains_key(&item.media_uuid) {
                let mut v = known_files.get_mut(&item.media_uuid).ok_or_else(|| {
                    anyhow::Error::msg("internal error: scan post-walk map missing key")
                })?;
                v.push(item.clone());
            } else if known_files
                .insert(item.media_uuid, vec![item.clone()])
                .is_some()
            {
                return Err(anyhow::Error::msg(
                    "internal error: scan post-walk map has duplicate key",
                ));
            }
        }

        Ok(known_files)
    }

    // determine which media file is most closely linked to a particular media record
    //
    // the precedence is:
    //  1) the original file, if its hash matches
    //  2) the oldest file with a matching hash
    //  3) the original path
    //
    // in the event of a hash and path match (corresponding to moving the original file but leaving
    // a new file in the orignal path), clone the record for the new file
    #[instrument(skip_all)]
    pub async fn resolve_duplicates(
        self: &Arc<Self>,
        media_uuid: MediaUuid,
        mut files: Vec<KnownFile>,
    ) -> Result<()> {
        debug!({media_uuid = media_uuid, file_count = files.len()}, "resolving duplicates");

        let context = self.clone();

        let (tx, rx) = tokio::sync::oneshot::channel();

        context
            .db_svc_sender
            .send(
                DbMsg::GetMedia {
                    resp: tx,
                    media_uuid,
                }
                .into(),
            )
            .await?;

        let (media, _, _) = rx.await??.ok_or_else(|| {
            anyhow::Error::msg(format!(
                "internal error: scan post-walk failed media lookup for {media_uuid}"
            ))
        })?;

        let parse_files = move |files: &Vec<KnownFile>| {
            // first check if the original object exists with a matching hash
            //
            // this corresponds to the trivial update, i.e. touched mtime
            for file in files.iter() {
                if media.chash == file.hash && media.path == file.path {
                    debug!({media_uuid = media_uuid, path = file.path}, "matched original file");
                    return Ok(file.clone());
                }
            }

            // next check if any object matches the original hash
            //
            // this corresponds to the file moving
            let mut real = None;
            let mut oldest = u64::MAX;

            for file in files.iter() {
                if media.chash == file.hash && file.mtime < oldest {
                    real = Some(file);
                    oldest = file.mtime;
                }
            }

            if let Some(file) = real {
                debug!({media_uuid = media_uuid, path = file.path}, "matched oldest moved file");
                return Ok(file.clone());
            }

            // finally, if there are no hash matches, check if the original
            // file still exists
            //
            // this corresponds to mutating the original file
            for file in files.iter() {
                if media.path == file.path {
                    debug!({media_uuid = media_uuid, path = file.path}, "matched original path");
                    return Ok(file.clone());
                }
            }

            // by construction, we shouldn't be able to reach this point
            Err(anyhow::Error::msg(format!(
                "internal error: scan post-walk failed to associate a file for {media_uuid}"
            )))
        };

        let real_file = parse_files(&files)?;

        // now that we have determined the real file, we need to update and possibly create records

        // first remove duplicates
        //
        // if the real file matched by hash, there may be duplicates elswewhere in the filesystem,
        // but if we matched by path then it will be unique in this vec
        files.pop_if(|f| f.hash == real_file.hash && !(f == &real_file));

        // then update the record to point to the new object, also removing it from the list
        //
        // if we matched by hash (i.e. moved original), then this will simply update the path
        // to point to the oldest file
        //
        // if we matched by path only, then this both updates the hash and mtime
        //
        // finally, if there are both hash and path matches, this will update the original
        // record to point to the hash match but there will still be the path match in the
        // dashset (and that should be the only remaining item)
        if let Some(file) = files.pop_if(|f| f == &real_file) {
            debug!({ media_uuid = media_uuid }, "updating media record");
            let (tx, rx) = tokio::sync::oneshot::channel();

            context
                .db_svc_sender
                .send(
                    DbMsg::ReplaceMediaPath {
                        resp: tx,
                        media_uuid: file.media_uuid,
                        path: file.path,
                        hash: file.hash,
                        mtime: file.mtime,
                    }
                    .into(),
                )
                .await?;

            rx.await??;

            // even though this is not a new file, the count starts at zero each run
            context.file_count.fetch_add(1, Ordering::Relaxed);
        } else {
            return Err(anyhow::Error::msg(
                "internal error: scan post-walk failed to find real file in set",
            ));
        }

        // there will be one lingering item in the vec if the original file was moved but a new file
        // was created in its place, i.e. matched by path and hash
        //
        // in this awkward corner case, we have to handle several issues

        // check that there is at most one path match, since we have removed all of the hash matches
        if files.len() > 1 {
            return Err(anyhow::Error::msg(format!(
                "internal error: scan post-walk had more than one path match for {media_uuid}"
            )));
        }

        debug!({media_uuid = media_uuid, path = real_file.path}, "updating original path after matching moved hash");

        if let Some(file) = files.pop() {
            // it's possible that the modified original path itself is a duplicate
            //
            // in that case, we simply add the clone tag and leave it, as there is no programatic
            // way to determine if the new record is newer or older than the original record

            let new_uuid = match context.get_media_by_chash(&file.hash).await? {
                Some(media) => media.media_uuid,
                None => {
                    let path = file.path.clone();

                    let scan = ScanFile::from_known(context.clone(), file).await?;

                    scan.register().await?.ok_or_else(|| {
                            anyhow::Error::msg(format!("internal error: new media record for {path} not added due to deduplication check"))
                        })?
                }
            };

            add_tag_to_media(
                context.db_svc_sender.clone(),
                new_uuid,
                format!("CLONE:{media_uuid}"),
            )
            .await?;

            context.file_count.fetch_add(1, Ordering::Relaxed);
        }

        Ok(())
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
    mtype: MediaType,
    pathstr: String,
    mtime: u64,
    metadata: Metadata,
    hash: String,
    scratch_dir: PathBuf,
}

impl Drop for ScanFile {
    fn drop(&mut self) {
        if std::fs::remove_dir_all(&self.scratch_dir).is_err() {
            let _span = span!(Level::INFO, "scan_file_drop", path = ?self.path).entered();
            warn!("failed to clean up scratch directory");
            self.context.warnings.fetch_add(1, Ordering::Relaxed);
        }
    }
}

impl ScanFile {
    pub async fn init(
        context: Arc<ScanContext>,
        path: PathBuf,
        metadata: Metadata,
    ) -> Result<FileStatus> {
        let context = context.clone();

        if !metadata.is_file() {
            return Err(anyhow::Error::msg(format!("{path:?} is not a file")));
        }

        let pathstr = path
            .to_str()
            .ok_or_else(|| anyhow::Error::msg("failed to convert path to str"))?
            .to_owned();

        let mtime = metadata.modified()?.duration_since(UNIX_EPOCH)?.as_secs();

        let mtype = match get_mtype(&path) {
            Ok(v) => v,
            Err(err) => {
                debug!({ path = pathstr }, "{err}");
                return Ok(FileStatus::Unknown);
            }
        };

        // matching path check
        //
        // the first way that a file can be linked to a record in the database is by path
        //
        // we can compare the mtime in the record with the file to know if the record is
        // up-to-date, which in turn allows us to skip the content hash
        let current = context.get_media_by_path(&pathstr).await?;

        if let Some(media) = current {
            if media.mtime >= mtime {
                return Ok(FileStatus::Skip);
            } else {
                debug!({media_uuid = media.media_uuid, path = pathstr}, "known media found via path");

                return Ok(FileStatus::Exists(KnownFile {
                    media_uuid: media.media_uuid,
                    path: pathstr.to_string(),
                    hash: content_hash(&path).await?,
                    mtime: media.mtime,
                }));
            }
        }

        // calculate the content hash of the file, which is the expensive step,
        // and use it to create a unique scratch directory
        let chash = content_hash(&path).await?;

        let scratch_dir = create_scratch_dir(context.clone(), &chash).await?;

        Ok(FileStatus::Register(ScanFile {
            context,
            path,
            mtype,
            pathstr,
            mtime,
            metadata,
            hash: chash,
            scratch_dir,
        }))
    }

    // in certain scenarios, we have to create a new media record for a file already
    // linked to another database record, and so we avoid recalculating hashes
    async fn from_known(context: Arc<ScanContext>, known: KnownFile) -> Result<Self> {
        let path = PathBuf::from(&known.path);
        let metadata = metadata(&path).await?;
        let mtype = get_mtype(&path)?;

        let scratch_dir = create_scratch_dir(context.clone(), &known.hash).await?;

        Ok(ScanFile {
            context,
            path,
            mtype,
            pathstr: known.path,
            mtime: known.mtime,
            metadata,
            hash: known.hash,
            scratch_dir,
        })
    }

    #[instrument(skip_all)]
    pub async fn register(&self) -> Result<Option<MediaUuid>> {
        debug!("processing media");

        // concurrent registration check
        //
        // we deduplicate by content hash while scanning and only record whichever file
        // the scanning threads found first
        //
        // however, this means that the other copies are not tracked for changes via the
        // known file checker; we could hypothetically extend the path in the database
        // to include all paths if this ends up becoming a problem
        if !self.context.chashes.insert(self.hash.clone()) {
            debug!("duplicate media found (concurrent check)");
            return Ok(None);
        }

        // matching hash check
        //
        // the second way that a file can be linked to a record in the database is by hash
        //
        // unlike the path mtime check, however, we cannot determine if a new path matching
        // an old hash is a moved file or a copy of a file whose original was edited, and
        // thus we have to compare the known files at the end
        let exists = self.context.get_media_by_chash(&self.hash).await?;

        if let Some(media) = exists {
            debug!({media_uuid = media.media_uuid, path = media.path}, "known media found via hash");

            self.context.known_files.insert(KnownFile {
                media_uuid: media.media_uuid,
                path: self.pathstr.to_string(),
                hash: self.hash.clone(),
                mtime: media.mtime,
            });

            return Ok(None);
        }

        // media processing
        let media_data: MediaData = match self.mtype {
            MediaType::Image => process_image(&self.path).await?,
            MediaType::Video => process_video(&self.path, &self.scratch_dir).await?,
        };

        // once we have the metadata, we assemble the Media struct and send it to the database
        let media = Media {
            library_uuid: self.context.library_uuid,
            path: self.pathstr.clone(),
            size: self.metadata.len(),
            chash: self.hash.clone(),
            phash: media_data.hash,
            mtime: self.mtime,
            hidden: false,
            date: media_data.date,
            note: "".to_owned(),
            tags: HashSet::new(),
            metadata: media_data.metadata.clone(),
        };

        // add the media to the database and get the uuid
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.context
            .db_svc_sender
            .send(DbMsg::AddMedia { resp: tx, media }.into())
            .await?;

        let media_uuid = rx.await??;

        self.install(media_uuid, media_data.metadata).await?;

        debug!("finished processing media");
        Ok(Some(media_uuid))
    }

    // media "installation"
    //
    // to actually access the media, we use symlinks.  this allows the http server to function like
    // an object store without needing to reorganize the filesystem. see also http/stream.rs.
    //
    // currently this part consists of two steps, but in principle any postprocessing needed to use
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

        let _ = remove_file(&thumbnail_path).await;

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
