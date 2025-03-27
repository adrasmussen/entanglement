use std::{
    fs::{canonicalize, metadata},
    path::PathBuf,
    sync::{
        atomic::{AtomicI64, Ordering},
        Arc,
    },
};

use anyhow::{Context, Result};
use tokio::task::JoinSet;
use tracing::{debug, error, info, instrument, span, warn, Instrument, Level};
use walkdir::WalkDir;

use crate::{
    db::msg::DbMsg,
    service::{ESMRegistry, ESMSender, ServiceType},
};
use api::library::LibraryUuid;
use common::config::ESConfig;

#[derive(Clone, Debug)]
struct ScanContext {
    db_svc_sender: ESMSender,
}

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
    let error_count = Arc::new(AtomicI64::new(0));

    let context = Arc::new(ScanContext { db_svc_sender });

    let mut tasks: JoinSet<()> = JoinSet::new();

    // for each entry in the directory tree, we will launch a new processing task into the joinset
    // after possibly waiting for some of previous tasks to clear up
    for entry in WalkDir::new(config.media_srcdir.clone().join(library.path))
        .same_file_system(true)
        .contents_first(true)
        .into_iter()
    {
        while tasks.len() > 8 {
            tasks.join_next().await;
        }

        let entry = match entry {
            Ok(entry) => entry,
            Err(ref err) => {
                warn!({entry = ?entry}, "failed to parse DirEntry: {err}");
                continue;
            }
        };

        let path = match canonicalize(entry.path()) {
            Ok(path) => path,
            Err(err) => {
                warn!({path = ?entry.path()}, "failed to canonicalize path: {err}");
                continue;
            }
        };

        let meta = match metadata(&path) {
            Ok(meta) => meta,
            Err(err) => {
                warn!({path = ?path}, "failed to parse metadata: {err}");
                continue;
            }
        };

        let context = context.clone();
        let error_count = error_count.clone();

        if meta.is_file() {
            tasks.spawn({
                // TODO -- fight the borrow checker to get path into the span data
                async move {
                match register_media(context, path).await {
                    Ok(()) => {}
                    Err(err) => {
                        warn!("scan error: {err}");
                        error_count.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }.instrument(span!(Level::INFO, "register_media"))});
        };
    }

    tasks.join_all().await;

    let error_count = match Arc::into_inner(error_count) {
        Some(v) => v.into_inner(),
        None => {
            error!("internal error: unpacking error_count Arc returned None");
            -1
        }
    };

    Ok(error_count)
}

// maybe this can return a uuid so that we can link in the fs_walker
#[instrument(skip(context))]
async fn register_media(context: Arc<ScanContext>, path: PathBuf) -> Result<()> {
    debug!("started processing media");

    std::fs::File::open("/does/not/exist").context("i'm a teapot")?;

    // rename to warning count

    todo!()
}
