use std::{
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    },
    time::Duration,
};

use anyhow::Result;
use dashmap::DashSet;

use tokio::{fs::create_dir_all, sync::oneshot::channel, task::JoinSet, time::timeout};
use tracing::{Instrument, Level, debug, error, instrument, span, warn, info};
use walkdir::WalkDir;

use crate::{
    db::msg::DbMsg,
    service::{ESMRegistry, ServiceType},
    task::scan_utils::{FileStatus, ScanContext, ScanFile, get_path_and_metadata},
};
use api::library::{LibraryUpdate, LibraryUuid};
use common::config::ESConfig;

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

    let (tx, rx) = channel();

    db_svc_sender
        .send(
            DbMsg::GetLibrary {
                resp: tx,
                library_uuid,
            }
            .into(),
        )
        .await?;

    let library = rx
        .await??
        .ok_or_else(|| anyhow::Error::msg("library does not exist"))?;

    let context = Arc::new(ScanContext {
        config: config.clone(),
        library_uuid,
        db_svc_sender: db_svc_sender.clone(),
        file_count: AtomicI64::new(0),
        warnings: AtomicI64::new(0),
        known_files: DashSet::new(),
        scratch_base: config
            .task
            .scan_scratch
            .clone()
            .join(library_uuid.to_string()),
    });

    create_dir_all(&context.scratch_base).await?;

    let mut tasks: JoinSet<()> = JoinSet::new();

    let scan_timeout = Duration::from_secs(context.config.task.scan_timeout);

    // for each entry in the directory tree, we will launch a new processing task into the joinset
    // after possibly waiting for some of previous tasks to clear up

    //
    // scan phase one
    //
    // in the first pass, we identify:
    //  * all new files
    //  * any modified files linked to a database record by path or hash
    //
    // everything else is skipped, incrementing the counter if it was a file
    // whose path is known and hasn't been modified
    info!("library scan phase one: filesystem walk and adding new media");

    for entry in WalkDir::new(config.fs.media_srcdir.clone().join(library.path))
        .same_file_system(true)
        .contents_first(true)
        .into_iter()
    {
        // check this first so that a database channel closure doesn't generate a ton of logs
        if context.db_svc_sender.is_closed() {
            error!("library scan task stopped -- database service cannot be reached");
            return Err(anyhow::Error::msg("database esm channel dropped"));
        };

        // we allow this to be configurable so that we don't swamp the media server when registering
        // a large collection of media
        while tasks.len() > config.task.scan_threads {
            tasks.join_next().await;
        }

        // process the media and register it with the database
        //
        // to allow for easy error propagation, we let register_media() return Result<()> and then
        // turn that error into a warning (since it won't actually stop the rest of the processing)
        //
        // importantly, those warnings should be attached to the span associated with path, so we
        // set up the span outside instead of using #[instrument]
        let (path, metadata) = get_path_and_metadata(entry).await?;

        if metadata.is_file() {
            tasks.spawn({
                // TODO -- why do these continues work?
                let context = context.clone();

                let file = match ScanFile::init(context.clone(), path.clone(), metadata).await? {
                    FileStatus::Register(v) => v,
                    FileStatus::Exists(file) => {
                        context.known_files.insert(file);
                        continue;
                    }
                    FileStatus::Skip => {
                        context.file_count.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                    FileStatus::Unknown => continue,
                };

                async move {
                    match timeout(scan_timeout, file.register())
                        .await
                        .map_err(|_| anyhow::Error::msg("scan exceeded timeout"))
                    {
                        Ok(Ok(Some(_))) => {
                            context.file_count.fetch_add(1, Ordering::Relaxed);
                        }
                        Ok(Ok(None)) => {}
                        Ok(Err(err)) | Err(err) => {
                            warn!("scan error: {err:?}");
                            context.warnings.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
                .instrument(span!(Level::INFO, "register_media", path = ?path))
            });
        }
    }

    // wait for phase one to complete
    tasks.join_all().await;

    //
    // scan phase two
    //
    // after having considered all files, we have a whole collection of linked records
    // that correspond to moved, changed, or duplicated files
    info!("library scan phase two: deduplicating and detecting changed media");

    // this early return is a bit cavalier, but this really shouldn't be able to fail
    let known_files = context.collate_known_files()?;

    let mut tasks: JoinSet<()> = JoinSet::new();

    for item in known_files.iter_mut() {
        let context = context.clone();
        let media_uuid = *item.key();
        let files = item.value().clone();

        // check this first so that a database channel closure doesn't generate a ton of logs
        if context.db_svc_sender.is_closed() {
            error!("library scan task stopped -- database service cannot be reached");
            return Err(anyhow::Error::msg("database esm channel dropped"));
        };

        // we allow this to be configurable so that we don't swamp the media server when registering
        // a large collection of media
        while tasks.len() > config.task.scan_threads {
            tasks.join_next().await;
        }

        tasks.spawn(async move {
            let context = context.clone();

            match timeout(scan_timeout, context.resolve_duplicates(media_uuid, files))
                .await
                .map_err(|_| anyhow::Error::msg("dedup exceeded timeout"))
            {
                Ok(Ok(())) => {}
                Ok(Err(err)) | Err(err) => {
                    warn!("dedup error: {err:?}");
                    context.warnings.fetch_add(1, Ordering::Relaxed);
                }
            }
        }.instrument(span!(Level::INFO, "dedup_media", media_uuid)));
    }

    // wait for phase two to complete
    tasks.join_all().await;

    let file_count = context.file_count.load(Ordering::Relaxed);
    let warnings = context.warnings.load(Ordering::Relaxed);

    // send the updated count to the library
    let (tx, rx) = channel();

    context
        .db_svc_sender
        .send(
            DbMsg::UpdateLibrary {
                resp: tx,
                library_uuid,
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
