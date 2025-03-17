use std::collections::HashSet;
use std::fmt::Display;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::config::ESConfig;

pub mod proxy;
pub mod tomlfile;

// probably want some sort of refresh method, as well as including exponential backoff
// on any impls that use network resources (and timeouts!)
//
// see notes in server/src/auth/svc.rs about why the is_group_member() can spam messages
//
// failures in the connect methods will prevent the server from starting correctly, and
// each provider should handle their own failures appropriately (unknown user, dropped
// external connection, etc).
#[async_trait]
pub trait AuthzBackend: Display + Send + Sync + 'static {
    async fn connect(config: Arc<ESConfig>) -> anyhow::Result<Self>
    where
        Self: Sized;

    async fn groups_for_user(&self, uid: String) -> HashSet<String>;

    async fn users_in_group(&self, gid: String) -> HashSet<String>;
}

#[async_trait]
pub trait AuthnBackend: Display + Send + Sync + 'static {
    async fn connect(config: Arc<ESConfig>) -> anyhow::Result<Self>
    where
        Self: Sized;

    async fn authenticate_user(&self, uid: String, password: String) -> bool;

    async fn is_valid_user(&self, uid: String) -> bool;
}
