use std::{
    collections::HashSet,
    fs::{canonicalize, metadata},
    hash::{DefaultHasher, Hash, Hasher},
    os::unix::fs::symlink,
    path::PathBuf,
    sync::{
        atomic::{AtomicI64, Ordering},
        Arc,
    },
};

use anyhow::Result;
use chrono::Local;
use tokio::{
    fs::{create_dir_all, remove_dir_all},
    task::JoinSet,
};
use tracing::{debug, error, instrument, span, warn, Instrument, Level};
use walkdir::WalkDir;

use crate::{
    db::msg::DbMsg,
    fs::{media_original_path, media_thumbnail_path},
    service::{ESMRegistry, ESMSender, ServiceType},
};
use api::{
    library::{LibraryUpdate, LibraryUuid},
    media::{Media, MediaMetadata},
};
use common::{
    config::ESConfig,
    media::{
        image::{create_image_thumbnail, process_image},
        video::{create_video_thumbnail, process_video},
        MediaData,
    },
};

#[derive(Clone, Debug)]
struct ScanContext {
    config: Arc<ESConfig>,
    library_uuid: LibraryUuid,
    db_svc_sender: ESMSender,
}

// library scanner task
//
// this task processes all of the media in a particular library concurrently and in parallel.  it
// walks the directory tree, extracting metadata and adding new media to the database, then adding
// symlinks so that transfer services can access the files.
//
// in its current implementation, the only critical failures (that return Err) are in the setup,
// or with the database connection -- any per-file problems are reported back as warnings.
#[instrument(skip(config, registry))]
pub async fn scan_library(
    config: Arc<ESConfig>,
    registry: ESMRegistry,
    library_uuid: LibraryUuid,
) -> Result<i64> {
    debug!("library scan pre-startup verification");

    let db_svc_sender = registry.get(&ServiceType::Db)?;

    let (tx, rx) = tokio::sync::oneshot::channel();

    db_svc_sender
        .send(
            DbMsg::GetLibrary {
                resp: tx,
                library_uuid: library_uuid,
            }
            .into(),
        )
        .await?;

    let library = rx
        .await??
        .ok_or_else(|| anyhow::Error::msg("library does not exist"))?;

    // create context construct to pass down into threads
    let file_count = Arc::new(AtomicI64::new(0));
    let warnings = Arc::new(AtomicI64::new(0));

    let context = Arc::new(ScanContext {
        config: config.clone(),
        library_uuid: library_uuid,
        db_svc_sender: db_svc_sender,
    });

    let mut tasks: JoinSet<()> = JoinSet::new();

    let scan_scratch = config.scan_scratch.join(library_uuid.to_string());

    // for each entry in the directory tree, we will launch a new processing task into the joinset
    // after possibly waiting for some of previous tasks to clear up
    for entry in WalkDir::new(config.media_srcdir.clone().join(library.path))
        .same_file_system(true)
        .contents_first(true)
        .into_iter()
    {
        // check this first so that a database channel closure doesn't generate a ton of logs
        if context.db_svc_sender.is_closed() {
            return Err(anyhow::Error::msg("database esm channel dropped"));
        };

        // we allow this to be configurable so that we don't swamp the media server when registering
        // a large collection of media
        while tasks.len() > config.scan_threads {
            tasks.join_next().await;
        }

        // under most normal circumstances/well-behaved filesystems, none of these operations should
        // fail, but we need to catch them regardless
        let entry = match entry {
            Ok(entry) => entry,
            Err(ref err) => {
                warn!({entry = ?entry}, "failed to parse DirEntry: {err}");
                warnings.fetch_add(1, Ordering::Relaxed);
                continue;
            }
        };

        let path = match canonicalize(entry.path()) {
            Ok(path) => path,
            Err(err) => {
                warn!({path = ?entry.path()}, "failed to canonicalize path: {err}");
                warnings.fetch_add(1, Ordering::Relaxed);
                continue;
            }
        };

        let meta = match metadata(&path) {
            Ok(meta) => meta,
            Err(err) => {
                warn!({path = ?path}, "failed to parse metadata: {err}");
                warnings.fetch_add(1, Ordering::Relaxed);
                continue;
            }
        };

        // clone the global state needed by register_media and its error handler
        let file_count = file_count.clone();
        let warnings = warnings.clone();
        let context = context.clone();

        // tasks have a scratch directory that is cleaned up on completion
        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);

        let file_scratch = scan_scratch.join(hasher.finish().to_string());

        // process the media and register it with the database
        //
        // to allow for easy error propagation, we let register_media() return Result<()> and then
        // turn that error into a warning (since it won't actually stop the rest of the processing)
        //
        // importantly, those warnings should be attached to the span associated with path, so we
        // set up the span outside instead of using #[instrument]
        if meta.is_file() {
            create_dir_all(&file_scratch).await?;

            tasks.spawn({
                let span = span!(Level::INFO, "register_media", path = ?path);
                async move {
                    match register_media(context, path, &file_scratch).await {
                        Ok(()) => {
                            file_count.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(err) => {
                            warn!("scan error: {err}");
                            warnings.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    match remove_dir_all(&file_scratch).await {
                        Ok(_) => {}
                        Err(err) => {
                            warn!({dir = ?file_scratch}, "failed to remove scratch directory: {err}");
                            warnings.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
                .instrument(span)
            });
        };
    }

    // cleanup
    tasks.join_all().await;

    remove_dir_all(scan_scratch).await?;

    let file_count = match Arc::into_inner(file_count) {
        Some(v) => v.into_inner(),
        None => {
            error!("internal error: unpacking file_count Arc returned None");
            -1
        }
    };

    let warnings = match Arc::into_inner(warnings) {
        Some(v) => v.into_inner(),
        None => {
            error!("internal error: unpacking warning Arc returned None");
            -1
        }
    };

    // send the updated count to the library
    let (tx, rx) = tokio::sync::oneshot::channel();

    context
        .db_svc_sender
        .send(
            DbMsg::UpdateLibrary {
                resp: tx,
                library_uuid: library_uuid,
                update: LibraryUpdate {
                    count: Some(file_count),
                },
            }
            .into(),
        )
        .await?;

    rx.await??;

    Ok(warnings)
}

// maybe this can return a uuid so that we can link in the fs_walker
async fn register_media(
    context: Arc<ScanContext>,
    path: PathBuf,
    scratchdir: &PathBuf,
) -> Result<()> {
    debug!("started processing media");

    // first, check if the media already exists in the database
    let pathstr = path
        .to_str()
        .ok_or_else(|| anyhow::Error::msg("failed to convert path to str"))?
        .to_owned();

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

    if rx.await??.is_some() {
        return Ok(());
    }

    // match the metadata collector via file extension
    let ext = path
        .extension()
        .map(|f| f.to_str())
        .flatten()
        .ok_or_else(|| anyhow::Error::msg("failed to extract file extention"))?;

    let media_data: MediaData = match ext {
        "jpg" | "png" | "tiff" => process_image(&path).await?,
        "mp4" => process_video(&path, &scratchdir).await?,
        _ => return Err(anyhow::Error::msg("no metadata collector for extension")),
    };

    // once we have the metadata, we assemble the Media struct and send it to the database
    let media = Media {
        library_uuid: context.library_uuid,
        path: pathstr.clone(),
        hash: media_data.hash,
        mtime: Local::now().timestamp(),
        hidden: false,
        date: media_data.date,
        note: "".to_owned(),
        tags: HashSet::new(),
        metadata: media_data.metadata.clone(),
    };

    let (tx, rx) = tokio::sync::oneshot::channel();

    context
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
        path.clone(),
        media_original_path(context.config.clone(), media_uuid),
    )?;

    let media_thumbnail_path = media_thumbnail_path(context.config.clone(), media_uuid);

    match media_data.metadata {
        MediaMetadata::Image => {
            Box::pin(create_image_thumbnail(&path, &media_thumbnail_path)).await?
        }
        MediaMetadata::Video => {
            Box::pin(create_video_thumbnail(
                &path,
                &media_thumbnail_path,
                &scratchdir,
            ))
            .await?
        }
        _ => return Err(anyhow::Error::msg("no thumbnail method found")),
    };

    debug!("finished processing media");
    Ok(())
}
