use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio;
use toml;

use crate::auth::{AuthnBackend, AuthzBackend};
use crate::config::ESConfig;

#[derive(Debug, Serialize, Deserialize)]
struct TomlUser {
    name: String,
    password: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TomlGroup {
    name: Option<String>,
    members: HashSet<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TomlAuthzFile {
    groups: HashMap<String, TomlGroup>,
}

#[async_trait]
impl AuthzBackend for TomlAuthzFile {
    async fn new(config: Arc<ESConfig>) -> Result<Self>
    where
        Self: Sized,
    {
        let filename = PathBuf::from(
            config
                .tomlfile
                .clone()
                .expect("tomlfile.filename not present")
                .filename,
        );

        let doc = tokio::fs::read_to_string(filename).await?;

        let data: Self = toml::from_str(&doc)?;

        Ok(data)
    }

    async fn groups_for_user(&self, uid: String) -> HashSet<String> {
        let mut gid = HashSet::new();

        for (k, v) in self.groups.iter() {
            if v.members.contains(&uid) {
                gid.insert(k.clone());
            }
        }

        gid
    }

    async fn users_in_group(&self, gid: String) -> HashSet<String> {
        match self.groups.get(&gid) {
            Some(v) => v.members.clone(),
            None => HashSet::new(),
        }
    }
}

impl Display for TomlAuthzFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "file-based group authorization")
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TomlAuthnFile {
    users: HashMap<String, TomlUser>,
}

#[async_trait]
impl AuthnBackend for TomlAuthnFile {
    async fn new(config: Arc<ESConfig>) -> Result<Self>
    where
        Self: Sized,
    {
        let filename = PathBuf::from(
            config
                .tomlfile
                .clone()
                .expect("tomlfile.filename not present")
                .filename,
        );

        let doc = tokio::fs::read_to_string(filename).await?;

        let data: Self = toml::from_str(&doc)?;

        Ok(data)
    }

    async fn authenticate_user(&self, uid: String, password: String) -> bool {
        match self.users.get(&uid) {
            None => false,
            Some(v) => match v.password.clone() {
                None => false,
                Some(v) => v == password,
            },
        }
    }

    async fn is_valid_user(&self, uid: String) -> bool {
        match self.users.get(&uid) {
            None => false,
            Some(_) => true,
        }
    }
}

impl Display for TomlAuthnFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "file-based group authentication")
    }
}
