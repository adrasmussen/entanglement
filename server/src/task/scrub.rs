use std::sync::Arc;

use anyhow::Result;

use common::config::ESConfig;
use crate::service::ESMRegistry;

pub async fn cache_scrub(_config: Arc<ESConfig>, _registry: ESMRegistry) -> Result<i64> {
    Ok(i64::MAX)
}
