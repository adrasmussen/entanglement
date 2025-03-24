use std::sync::Arc;

use anyhow::Result;

use crate::service::{ESMRegistry, ServiceType};
use api::{library::{Library, LibraryUuid}, task::TaskUuid};
use common::config::ESConfig;

pub async fn scan_library(
    config: Arc<ESConfig>,
    registry: ESMRegistry,
    library_uuid: LibraryUuid
) -> Result<()> {
    let db_svc_sender = registry.get(&ServiceType::Db)?;


    // create context construct to pass down into threads

    Ok(())
}
