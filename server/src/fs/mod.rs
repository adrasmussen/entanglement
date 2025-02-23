use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow;
use api::THUMBNAIL_PATH;
use async_trait::async_trait;

use crate::service::*;
use api::{
    library::{LibraryScanJob, LibraryUuid},
    ORIGINAL_PATH,
};
use common::config::ESConfig;

pub mod clean;
pub mod msg;
pub mod scan;
pub mod svc;

#[async_trait]
pub trait ESFileService: ESInner {
    async fn scan_library(&self, library_uuid: LibraryUuid) -> anyhow::Result<()>;

    async fn scan_status(&self) -> anyhow::Result<HashMap<LibraryUuid, LibraryScanJob>>;

    async fn stop_scan(&self, library_uuid: LibraryUuid) -> anyhow::Result<()>;

    async fn fix_symlinks(&self) -> anyhow::Result<()>;
}

fn media_original_path(config: Arc<ESConfig>, media_uuid: String) -> PathBuf {
    config
        .media_srvdir
        .join(ORIGINAL_PATH)
        .join(media_uuid.to_string())
}

fn media_thumbnail_path(config: Arc<ESConfig>, media_uuid: String) -> PathBuf {
    config
        .media_srvdir
        .join(THUMBNAIL_PATH)
        .join(media_uuid.to_string())
}
