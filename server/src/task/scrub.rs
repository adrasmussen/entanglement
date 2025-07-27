use std::{
    collections::HashSet,
    fs::Metadata,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    },
};

use anyhow::Result;
use tokio::{
    fs::{remove_dir_all, remove_file},
    sync::oneshot::channel,
};
use tracing::{debug, error, instrument, warn};
use walkdir::{DirEntry, WalkDir};

use crate::{
    db::msg::DbMsg,
    service::{ESMRegistry, ServiceType},
    task::scan_utils::get_path_and_metadata,
};
use api::{LINK_PATH, THUMBNAIL_PATH, media::MediaUuid};
use common::config::ESConfig;

#[instrument(skip_all)]
pub async fn cache_scrub(config: Arc<ESConfig>, registry: ESMRegistry) -> Result<i64> {
    debug!("media cache clean pre-startup verification");

    let db_svc_sender = registry.get(&ServiceType::Db)?;

    let (tx, rx) = channel();

    db_svc_sender
        .send(DbMsg::GetMediaUuids { resp: tx }.into())
        .await?;

    let media_uuids = rx.await??.into_iter().collect::<HashSet<MediaUuid>>();

    let warnings = Arc::new(AtomicI64::new(0));

    // remove any links that do not reference a valid media_uuid
    debug!("scrubbing link cache");

    for entry in WalkDir::new(config.fs.media_srvdir.clone().join(LINK_PATH))
        .same_file_system(true)
        .max_depth(1)
        .into_iter()
    {
        if let Err(err) = scrub_link(entry, &media_uuids).await {
            warn!("link scrub error: {err}");
            warnings.fetch_add(1, Ordering::Relaxed);
        }
    }

    // remove any thumbnails that do not reference a valid media_uuid
    debug!("scrubbing thumbnail cache");

    for entry in WalkDir::new(config.fs.media_srvdir.clone().join(THUMBNAIL_PATH))
        .same_file_system(true)
        .max_depth(1)
        .into_iter()
    {
        if let Err(err) = scrub_thumbnail(entry, &media_uuids).await {
            warn!("thumbnail scrub error: {err}");
            warnings.fetch_add(1, Ordering::Relaxed);
        }
    }

    let warnings = warnings.load(Ordering::Relaxed);

    Ok(warnings)
}

async fn remove_path(path: &Path, metadata: &Metadata) -> Result<()> {
    if metadata.is_file() || metadata.is_symlink() {
        remove_file(path).await?
    } else if metadata.is_dir() {
        remove_dir_all(path).await?
    } else {
        error!("{path:?} is neither file nor directory");
        return Err(anyhow::Error::msg(format!(
            "internal error: {path:?} is neither file nor directory"
        )));
    }
    Ok(())
}

fn valid_uuid(path: &Path, media_uuids: &HashSet<MediaUuid>) -> bool {
    match path.file_name().map(|s| s.to_string_lossy().parse::<u64>()) {
        Some(Ok(v)) => media_uuids.contains(&v),
        _ => false,
    }
}

// cache scrubbers
//
// while these are very similar, we want to allow for differing logic when
// cleaning the individual caches instead of having one big match statement
#[instrument(skip_all)]
async fn scrub_link(
    entry: walkdir::Result<DirEntry>,
    media_uuids: &HashSet<MediaUuid>,
) -> Result<()> {
    let (path, metadata) = get_path_and_metadata(entry).await?;

    if !(metadata.is_symlink() && valid_uuid(&path, media_uuids)) {
        debug!("removing {path:?}");
        remove_path(&path, &metadata).await?;
    }

    Ok(())
}

#[instrument(skip_all)]
async fn scrub_thumbnail(
    entry: walkdir::Result<DirEntry>,
    media_uuids: &HashSet<MediaUuid>,
) -> Result<()> {
    let (path, metadata) = get_path_and_metadata(entry).await?;

    if !(metadata.is_file() && valid_uuid(&path, media_uuids)) {
        debug!("removing {path:?}");
        remove_path(&path, &metadata).await?;
    }

    Ok(())
}
