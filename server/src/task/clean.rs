use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    },
};

use anyhow::Result;
use tokio::{
    fs::{create_dir_all, metadata, remove_dir_all, remove_file, symlink, try_exists},
    sync::oneshot::channel,
    task::JoinSet,
};
use tracing::{debug, instrument, warn, span, Level};

use crate::{
    db::msg::DbMsg,
    fs::{media_link_path, media_thumbnail_path},
    service::{ESMRegistry, EsmSender, ServiceType},
};
use api::{
    library::{LibraryUpdate, LibraryUuid},
    media::{MediaUpdate, MediaUuid},
    search::SearchFilter,
};
use common::{config::ESConfig, media::create_thumbnail};

#[derive(Debug)]
pub struct CleanContext {
    pub config: Arc<ESConfig>,
    pub db_svc_sender: EsmSender,
    pub scratch_base: PathBuf,
}

impl Drop for CleanContext {
    fn drop(&mut self) {
        if std::fs::remove_dir_all(&self.scratch_base).is_err() {
            let _ = span!(Level::INFO, "clean_context_drop").entered();
            warn!("failed to clean up scratch base directory");
        }
    }
}

#[instrument(skip(config, registry))]
pub async fn clean_library(
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

    let context = Arc::new(CleanContext {
        config: config.clone(),
        db_svc_sender: db_svc_sender.clone(),
        scratch_base: config
            .task
            .scan_scratch
            .clone()
            .join(library_uuid.to_string()),
    });

    let warnings = Arc::new(AtomicI64::new(0));

    create_dir_all(&context.scratch_base).await?;

    let mut tasks: JoinSet<()> = JoinSet::new();

    // TODO -- rename or use separate name
    let clean_threads = config.task.scan_threads;

    debug!("library clean beginning database walk");

    for media_uuid in media_in_library {
        while tasks.len() > clean_threads {
            tasks.join_next().await;
        }

        tasks.spawn({
            let context = context.clone();
            let warnings = warnings.clone();

            async move {
                match clean_media(context, media_uuid).await {
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

    remove_dir_all(&context.scratch_base).await?;

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

#[instrument(skip(context))]
async fn clean_media(context: Arc<CleanContext>, media_uuid: MediaUuid) -> Result<()> {
    let config = context.config.clone();
    let db_svc_sender = context.db_svc_sender.clone();

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

    let media = rx
        .await??
        .ok_or_else(|| {
            anyhow::Error::msg("internal error: failed to get_media after searching library")
        })?
        .0;

    let path = PathBuf::from(media.path);

    // original media validation
    //
    // while we could have the cleaner job delete any db entries for media that has vanished,
    // it would lead to moved media being deleted if the scan wasn't called first.  thus, we
    // tag it and have a separate task to clean those entries out.
    debug!("original media validation");

    if !try_exists(&path).await? {
        let (tx, rx) = tokio::sync::oneshot::channel();

        let mut tags = media.tags.clone();

        tags.insert("TBD_DELETEME".to_owned());

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

        rx.await??;

        return Err(anyhow::Error::msg("missing media"));
    }

    let path_metadata = metadata(&path).await?;

    // symlink validation and cleanup
    //
    // the symlink folder is effective a cache for the media path column, and this
    // ensures that the links into the given library are all accurrate
    //
    // there is a separate task (that can run concurrently) that removes unknown
    // symlinks, since no library will attempt to modify them via this task
    let link_path = media_link_path(config.clone(), media_uuid);

    let mut relink = false;

    // early return handles the case where we cannot verify if the link exists
    if try_exists(&link_path).await? {
        let link_metadata = metadata(&link_path).await?;

        if !link_metadata.is_symlink()
            || link_path.canonicalize()? != media_link_path(config.clone(), media_uuid)
        {
            relink = true;
        }
    } else {
        relink = true;
    }

    if relink {
        match remove_file(&link_path).await {
            Ok(()) => {}
            Err(err) => match err.kind() {
                std::io::ErrorKind::NotFound => {}
                _ => return Err(anyhow::Error::from(err)),
            },
        }

        symlink(&path, &link_path).await?;
    }

    // thumbnail validation and cleanup
    //
    // we need to replace the thumbnails if they don't exist or the media changes
    let thumbnail_path = media_thumbnail_path(config.clone(), media_uuid);

    let mut regen = false;

    if try_exists(&thumbnail_path).await? {
        let thumnail_metadata = metadata(&thumbnail_path).await?;

        if path_metadata.modified()? > thumnail_metadata.modified()? {
            regen = true;
        }
    } else {
        regen = true;
    }

    if regen {
        remove_file(&thumbnail_path).await?;

        let scratch_dir = context.scratch_base.join(media_uuid.to_string());

        create_dir_all(&scratch_dir).await?;

        create_thumbnail(&path, &thumbnail_path, &scratch_dir, &media.metadata).await?;

        remove_dir_all(scratch_dir).await?;
    }

    Ok(())
}
