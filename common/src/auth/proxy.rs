use std::{fmt::Display, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;

use crate::auth::AuthnBackend;
use crate::config::ESConfig;

pub struct ProxyAuth {}

#[async_trait]
impl AuthnBackend for ProxyAuth {
    fn new(_config: Arc<ESConfig>) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(ProxyAuth {})
    }

    async fn authenticate_user(&self, _uid: String, _password: String) -> bool {
        true
    }

    // TODO -- coordinate with the middleware to actually verify things
    async fn is_valid_user(&self, _uid: String) -> bool {
        true
    }
}

impl Display for ProxyAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "http reverse proxy header authentication")
    }
}
