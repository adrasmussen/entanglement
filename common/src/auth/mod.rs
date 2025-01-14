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

// probably want some sort of refresh method
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

    async fn is_valid_user(&self, uid: String, password: String) -> anyhow::Result<bool>;
}
