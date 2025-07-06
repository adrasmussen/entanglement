use std::{fmt::Display, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::auth::AuthnProvider;
use crate::config::ESConfig;

// http mutual tls/x509 certificate auth
//
// see server::http::auth.rs for the middleware that implements the logic
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CertAuthnConfig {}

#[derive(Debug)]
pub struct CertAuthn {}

#[async_trait]
impl AuthnProvider for CertAuthn {
    fn new(_config: Arc<ESConfig>) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(CertAuthn {})
    }

    async fn authenticate_user(&self, _uid: String, _password: String) -> Result<bool> {
        Ok(true)
    }

    async fn is_valid_user(&self, _uid: String) -> Result<bool> {
        Ok(true)
    }
}

impl Display for CertAuthn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "http x509/mtls authentication")
    }
}
