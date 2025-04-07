use std::{collections::HashSet, fmt::Display, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;

use crate::config::ESConfig;

pub mod proxy;
pub mod tomlfile;

// probably want some sort of refresh method, as well as including exponential backoff
// on any impls that use network resources (and timeouts!)
//
// see notes in server/src/auth/svc.rs about why the is_group_member() can spam messages
//
// TODO -- make all of these functions correctly falliable, and ensure that they have
// some sort of retry logic built in
#[async_trait]
pub trait AuthzBackend: Display + Send + Sync + 'static {
    fn new(config: Arc<ESConfig>) -> Result<Self>
    where
        Self: Sized;

    async fn groups_for_user(&self, uid: String) -> Result<HashSet<String>>;

    async fn users_in_group(&self, gid: String) -> Result<HashSet<String>>;
}

#[async_trait]
pub trait AuthnBackend: Display + Send + Sync + 'static {
    fn new(config: Arc<ESConfig>) -> Result<Self>
    where
        Self: Sized;

    async fn authenticate_user(&self, uid: String, password: String) -> Result<bool>;

    async fn is_valid_user(&self, uid: String) -> Result<bool>;
}
