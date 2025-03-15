use std::sync::Arc;

use anyhow::Result;

use crate::service::ESMRegistry;
use api::library::Library;
use common::config::ESConfig;

pub async fn scan_library(config: Arc<ESConfig>, registry: ESMRegistry, library: Library) -> Result<()> {Ok(())}
