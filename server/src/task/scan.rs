use std::sync::{
    atomic::{AtomicI64, Ordering},
    Arc,
};

use anyhow::Result;
use dashmap::DashSet;

use tokio::{
    fs::{create_dir_all, remove_dir_all},
    task::JoinSet,
};
use tracing::{debug, instrument, span, warn, Instrument, Level};
use walkdir::WalkDir;

use crate::{
    db::msg::DbMsg,
    service::{ESMRegistry, ServiceType},
    task::scan_utils::{get_path_and_metadata, ScanFile, ScanContext},
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

    let context = Arc::new(ScanContext {
        config: config.clone(),
        library_uuid: library_uuid,
        db_svc_sender: db_svc_sender,
        file_count: AtomicI64::new(0),
        warnings: AtomicI64::new(0),
        chashes: DashSet::new(),
        scratch_base: config
            .task
            .scan_scratch
            .clone()
            .join(library_uuid.to_string()),
    });

    create_dir_all(&context.scratch_base).await?;

    let mut tasks: JoinSet<()> = JoinSet::new();

    let scan_threads = config.task.scan_threads.clone();

    // for each entry in the directory tree, we will launch a new processing task into the joinset
    // after possibly waiting for some of previous tasks to clear up
    for entry in WalkDir::new(config.fs.media_srcdir.clone().join(library.path))
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
        while tasks.len() > scan_threads {
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
        let file = match ScanFile::new(context.clone(), path.clone(), metadata).await? {
            Some(v) => v,
            None => {
                context.file_count.fetch_add(1, Ordering::Relaxed);
                continue;
            }
        };

        tasks.spawn({
            let context = context.clone();

            async move {
                match file.timed_register().await {
                    Ok(()) => {
                        context.file_count.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(err) => {
                        warn!("scan error: {err:?}");
                        context.warnings.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
            .instrument(span!(Level::INFO, "register_media", path = ?path))
        });
    }}

    // cleanup
    tasks.join_all().await;

    remove_dir_all(&context.scratch_base).await?;

    let file_count = context.file_count.load(Ordering::Relaxed);
    let warnings = context.warnings.load(Ordering::Relaxed);

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
