use std::sync::Arc;

use async_trait::async_trait;

use crate::auth::AuthnBackend;
use crate::config::ESConfig;

pub struct ProxyAuth {}

#[async_trait]
impl AuthnBackend for ProxyAuth {
    async fn connect(_config: Arc<ESConfig>) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        Ok(ProxyAuth {})
    }

    async fn authenticate_user(&self, _uid: String, _password: String) -> anyhow::Result<bool> {
        Ok(true)
    }

    // in reality, this should somehow communicate with the proxy layer to check if a user has
    // previously logged on
    async fn is_valid_user(&self, _uid: String) -> anyhow::Result<bool> {
        Ok(true)
    }
}
