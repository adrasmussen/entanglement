use std::collections::HashMap;

use anyhow;
use async_trait::async_trait;

use common::api::library::{LibraryScanJob, LibraryUuid};

use crate::service::*;

pub mod msg;
pub mod scan;
pub mod svc;

#[async_trait]
pub trait ESFileService: ESInner {
    async fn scan_library(&self, library_uuid: LibraryUuid) -> anyhow::Result<()>;

    async fn scan_status(&self) -> anyhow::Result<HashMap<LibraryUuid, LibraryScanJob>>;

    async fn fix_symlinks(&self) -> anyhow::Result<()>;
}
