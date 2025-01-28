use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use saphyr::Yaml;

use crate::auth::AuthzBackend;
use crate::config::ESConfig;

// group: members
pub struct YamlGroupFile {
    data: HashMap<String, HashSet<String>>,
}

#[async_trait]
impl AuthzBackend for YamlGroupFile {
    async fn connect(config: Arc<ESConfig>) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let filename = PathBuf::from(
            config
                .authz_yaml_groups
                .clone()
                .ok_or_else(|| anyhow::Error::msg("invalid group yaml filename"))?,
        );

        let yaml_string = std::fs::read_to_string(filename.clone())?;

        let yaml_docs = Yaml::load_from_str(&yaml_string)?;

        let yaml_doc = yaml_docs
            .get(0)
            .ok_or_else(|| anyhow::Error::msg("no valid yaml documents in group yaml file"))?
            .clone();

        let yaml_hash = match yaml_doc {
            Yaml::Hash(val) => val,
            _ => {
                return Err(anyhow::Error::msg(
                    "invalid yaml format, expected a hash map in the first document",
                ))
            }
        };

        let mut data = HashMap::<String, HashSet<String>>::new();

        for (k, v) in yaml_hash.iter() {
            let k = k
                .clone()
                .into_string()
                .ok_or_else(|| anyhow::Error::msg("invalid key in group yaml file"))?;

            let v = v
                .clone()
                .into_vec()
                .ok_or_else(|| anyhow::Error::msg("failed to parse group yaml"))?
                .iter()
                .map(|e| {
                    e.clone()
                        .into_string()
                        .ok_or_else(|| anyhow::Error::msg("failed to parse group yaml"))
                })
                .collect::<anyhow::Result<HashSet<String>>>()?;

            if data.contains_key(&k) {
                return Err(anyhow::Error::msg("duplicate group found in group yaml"));
            }
            data.insert(k, v);
        }

        Ok(YamlGroupFile { data: data })
    }

    async fn groups_for_user(&self, uid: String) -> anyhow::Result<HashSet<String>> {
        let mut out = HashSet::new();

        for (k, v) in self.data.iter() {
            if v.contains(&uid) {
                out.insert(k.clone());
            }
        }

        Ok(out)
    }

    async fn users_in_group(&self, gid: String) -> anyhow::Result<HashSet<String>> {
        match self.data.get(&gid) {
            Some(v) => Ok(v.clone()),
            None => Ok(HashSet::new()),
        }
    }
}

impl Display for YamlGroupFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "file-based group authorization")
    }
}
