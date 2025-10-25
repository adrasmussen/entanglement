use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    path::PathBuf,
    sync::Arc,
};

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::fs::read_to_string;
use toml;
use tracing::{info, instrument};

use crate::auth::{AuthnProvider, AuthzProvider};
use crate::config::ESConfig;

// toml file authentication and authorization
//
// this is the simplest possible static database of users and groups
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TomlFileConfig {
    pub filename: PathBuf,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TomlAuthnFile {
    filename: PathBuf,
}

#[derive(Debug, Deserialize, Serialize)]
struct TomlUser {
    name: String,
    password: Option<String>,
}

impl TomlAuthnFile {
    async fn parse(&self) -> Result<HashMap<String, TomlUser>> {
        let doc = read_to_string(&self.filename).await?;

        #[derive(Debug, Deserialize, Serialize)]
        struct TomlData {
            users: HashMap<String, TomlUser>,
        }

        let data: TomlData = toml::from_str(&doc)?;

        Ok(data.users)
    }
}

#[async_trait]
impl AuthnProvider for TomlAuthnFile {
    #[instrument(skip_all)]
    fn new(config: Arc<ESConfig>) -> Result<Self>
    where
        Self: Sized,
    {
        info!("reading toml file for authn");

        let filename = config
            .tomlfile
            .clone()
            .expect("tomlfile.filename not present")
            .filename;

        Ok(TomlAuthnFile { filename })
    }

    async fn authenticate_user(&self, uid: String, password: String) -> Result<bool> {
        let users = self.parse().await?;

        let res = match users.get(&uid) {
            None => false,
            Some(v) => match v.password.clone() {
                None => false,
                Some(v) => v == password,
            },
        };

        Ok(res)
    }

    async fn is_valid_user(&self, uid: String) -> Result<bool> {
        let users = self.parse().await?;

        Ok(users.contains_key(&uid))
    }
}

impl Display for TomlAuthnFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "file-based group authentication")
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TomlAuthzFile {
    filename: PathBuf,
}

#[derive(Debug, Deserialize, Serialize)]
struct TomlGroup {
    name: Option<String>,
    members: HashSet<String>,
}

impl TomlAuthzFile {
    #[instrument(skip_all)]
    async fn parse(&self) -> Result<HashMap<String, TomlGroup>> {
        let doc = read_to_string(&self.filename).await?;

        #[derive(Debug, Deserialize, Serialize)]
        struct TomlData {
            groups: HashMap<String, TomlGroup>,
        }

        let data: TomlData = toml::from_str(&doc)?;

        Ok(data.groups)
    }
}

#[async_trait]
impl AuthzProvider for TomlAuthzFile {
    #[instrument(skip_all)]
    fn new(config: Arc<ESConfig>) -> Result<Self>
    where
        Self: Sized,
    {
        info!("reading toml file for authz");

        let filename = config
            .tomlfile
            .clone()
            .expect("tomlfile.filename not present")
            .filename;

        Ok(TomlAuthzFile { filename })
    }

    async fn groups_for_user(&self, uid: String) -> Result<HashSet<String>> {
        let mut gid = HashSet::new();

        let groups = self.parse().await?;

        for (k, v) in groups.iter() {
            if v.members.contains(&uid) {
                gid.insert(k.clone());
            }
        }

        Ok(gid)
    }

    async fn users_in_group(&self, gid: String) -> Result<HashSet<String>> {
        let groups = self.parse().await?;

        let res = match groups.get(&gid) {
            Some(v) => v.members.clone(),
            None => HashSet::new(),
        };

        Ok(res)
    }
}

impl Display for TomlAuthzFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "file-based group authorization")
    }
}
