use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;

use crate::config::ESConfig;

pub mod yamlfile;

pub struct User {
    pub uid: String,
    pub name: String,
    pub groups: HashSet<String>,
}

pub struct Group {
    pub gid: String,
    pub name: String,
    pub members: HashSet<String>,
}

// probably want some sort of refresh method, as well as including exponential backoff
// on any impls that use network resources (and timeouts!)
//
// see notes in server/src/auth/svc.rs about why the is_group_member() can spam messages
#[async_trait]
pub trait AuthzBackend: Send + Sync + 'static {
    async fn connect(config: Arc<ESConfig>) -> anyhow::Result<Self>
    where
        Self: Sized;

    async fn is_group_member(&self, uid: String, gid: String) -> anyhow::Result<bool>;
}

#[async_trait]
pub trait AuthnBackend: Send + Sync + 'static {
    async fn connect(config: Arc<ESConfig>) -> anyhow::Result<Self>
    where
        Self: Sized;

    async fn authenticate_user(&self, uid: String, password: String) -> anyhow::Result<bool>;

    async fn is_valid_user(&self, uid: String) -> anyhow::Result<bool>;
}
