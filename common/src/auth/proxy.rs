use std::{fmt::Display, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;

use crate::auth::AuthnBackend;
use crate::config::ESConfig;

// reverse proxy authentication
//
// stub module for when all http calls are handled via the reverse proxy
#[derive(Debug)]
pub struct ProxyAuth {}

#[async_trait]
impl AuthnBackend for ProxyAuth {
    fn new(_config: Arc<ESConfig>) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(ProxyAuth {})
    }

    async fn authenticate_user(&self, _uid: String, _password: String) -> Result<bool> {
        Ok(true)
    }

    // TODO -- coordinate with the middleware to actually verify things
    async fn is_valid_user(&self, _uid: String) -> Result<bool> {
        Ok(true)
    }
}

impl Display for ProxyAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "http reverse proxy header authentication")
    }
}
