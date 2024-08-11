use anyhow;

use async_trait::async_trait;

use api::library::{LibraryUuid, LibraryScanResult};

use crate::service::*;

pub mod msg;
pub mod scan;
pub mod svc;

#[async_trait]
pub trait ESFileService: ESInner {
    async fn scan_library(&self, library_uuid: LibraryUuid) -> anyhow::Result<LibraryScanResult>;

    async fn fix_symlinks(&self) -> anyhow::Result<()>;
}
