use std::sync::Arc;

use anyhow::Result;
use tokio::task::JoinHandle;

use crate::service::{ESMRegistry, ESMSender, ServiceType};
use api::library::Library;
use common::config::ESConfig;

pub async fn scan_library(
    config: Arc<ESConfig>,
    registry: ESMRegistry,
    library: Library,
) -> Result<()> {
    // create context construct to pass down into threads

    Ok(())
}
