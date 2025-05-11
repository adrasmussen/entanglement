use std::{
    collections::HashSet,
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    },
};

use anyhow::Result;
use dashmap::DashSet;

use tokio::{
    fs::{create_dir_all, metadata, remove_dir_all, try_exists},
    sync::oneshot::channel,
    task::JoinSet,
};
use tracing::{Instrument, Level, debug, error, info, instrument, span, warn};
use walkdir::WalkDir;

use crate::{
    db::msg::DbMsg, fs::media_thumbnail_path, service::{ESMRegistry, ServiceType}
};
use api::{
    library::{LibraryUpdate, LibraryUuid},
    media::MediaUuid,
    search::SearchFilter,
};
use common::config::ESConfig;

// restore missing symlinks for media that exist
//
// regenerate thumbnails based on mtime
//
// remove symlinks/db entries for media that no
// longer exists
//
// add "CLEANER:duplicate" tag for anything that
// matches very closely --> move to dedup task?
#[instrument(skip(config, registry))]
pub async fn clean(
    config: Arc<ESConfig>,
    registry: ESMRegistry,
    library_uuid: LibraryUuid,
) -> Result<i64> {
    debug!("library clean pre-startup verification");

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

    let (tx, rx) = channel();

    db_svc_sender
        .send(
            DbMsg::SearchMediaInLibrary {
                resp: tx,
                gid: HashSet::from([library.gid]),
                library_uuid,
                hidden: None,
                filter: SearchFilter::default(),
            }
            .into(),
        )
        .await?;

    let media_in_library = rx.await??;

    let warnings = Arc::new(AtomicI64::new(0));

    let mut tasks: JoinSet<()> = JoinSet::new();

    // TODO -- rename or use separate name
    let clean_threads = config.task.scan_threads;

    debug!("library clean beginning database walk");

    for media_uuid in media_in_library {
        while tasks.len() > clean_threads {
            tasks.join_next().await;
        }

        tasks.spawn({
            let config = config.clone();
            let warnings = warnings.clone();

            async move {
                match clean_media(config, media_uuid).await {
                    Ok(()) => {}
                    Err(err) => {
                        warn!("clean error: {err:?}");
                        warnings.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        });
    }

    tasks.join_all().await;

    let warnings = warnings.load(Ordering::Relaxed);

    // update the mtime with this embarassing hack
    //
    // TODO -- consider letting empty updates still bump the mtime (on everything)
    let (tx, rx) = channel();

    db_svc_sender
        .send(
            DbMsg::UpdateLibrary {
                resp: tx,
                library_uuid,
                update: LibraryUpdate {
                    count: Some(library.count),
                },
            }
            .into(),
        )
        .await?;

    rx.await??;

    Ok(warnings)
}

#[instrument]
async fn clean_media(config: Arc<ESConfig>, media_uuid: MediaUuid) -> Result<()> {
    // thumbnails

    let link_metadata = metadata(media_thumbnail_path(config.clone(), media_uuid)).await;

    // if we first ensure that all of the symlinks are good (i.e. clean out the link dir)
    // for anything that doesn't exist
    if !try_exists(media_thumbnail_path(config, media_uuid)).await? {

    }



    Ok(())
}
