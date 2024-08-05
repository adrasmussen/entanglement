use std::path::PathBuf;

use anyhow;

use async_trait::async_trait;

use api::library::*;

use crate::fs::scan::ScanReport;
use crate::service::*;

pub mod msg;
pub mod scan;
pub mod svc;

#[async_trait]
pub trait ESFileService: ESInner {
    async fn scan_library(&self, library_uuid: LibraryUuid) -> anyhow::Result<ScanReport>;

    async fn fix_symlinks(&self) -> anyhow::Result<()>;
}

fn user_to_library(user: String, media_srcdir: PathBuf) -> anyhow::Result<PathBuf> {
    let mut root = media_srcdir.clone();

    if !root.is_absolute() {
        return Err(anyhow::Error::msg("media_srcdir path must be absolute"));
    }

    root.push("libraries");
    root.push(user);

    Ok(root)
}
