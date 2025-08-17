use std::{fmt::Display, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::auth::AuthnProvider;
use crate::config::ESConfig;

// reverse proxy authentication
//
// see server::http::auth.rs for the middleware that implements the logic
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProxyHeaderConfig {
    // the header passed down from the proxy, must be all lowercase
    pub header: String,
    // the subject cn field in the client x509 cert used by the proxy
    pub proxy_cn: String,
}

#[derive(Debug)]
pub struct ProxyAuth {}

#[async_trait]
impl AuthnProvider for ProxyAuth {
    fn new(_config: Arc<ESConfig>) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(ProxyAuth {})
    }

    async fn authenticate_user(&self, _uid: String, _password: String) -> Result<bool> {
        Ok(true)
    }

    async fn is_valid_user(&self, _uid: String) -> Result<bool> {
        Ok(true)
    }
}

impl Display for ProxyAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "http reverse proxy header authentication")
    }
}
