use std::{path::PathBuf, sync::Arc};

use serde::{Deserialize, Serialize};
use tokio;
use toml;
use tracing::{Level, debug, instrument};

use crate::{
    auth::{ldap::LdapConfig, proxy::ProxyHeaderConfig, tomlfile::TomlFileConfig, gss::GssConfig},
    db::mariadb::MariaDbConfig,
    server::{FsConfig, HttpConfig, TaskConfig},
};

// entanglement configuration
//
// this struct contains all of the myriad configuration options used by the server and cli tools
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ESConfig {
    pub authn_backend: AuthnBackend,
    pub authz_backend: AuthzBackend,
    pub db_backend: DbBackend,

    // core services
    pub fs: FsConfig,
    pub http: HttpConfig,
    pub task: TaskConfig,

    // backends
    pub gss: Option<GssConfig>,
    pub ldap: Option<LdapConfig>,
    pub mariadb: Option<MariaDbConfig>,
    pub tomlfile: Option<TomlFileConfig>,
    pub proxyheader: Option<ProxyHeaderConfig>,
}

// backends
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum AuthnBackend {
    // header set by reverse proxy
    ProxyHeader,
    // a toml file with usernames and passwords
    TomlFile,
    // subject cn data in certificate
    X509Cert,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum AuthzBackend {
    // standard ldap3 auth
    Ldap,
    // a toml file with group memberships
    TomlFile,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum DbBackend {
    MariaDB,
}

// in order to extract the config table from a larger document, we need to specify it
// as a subtable of the root node, i.e. a substruct
#[derive(Debug, Deserialize, Serialize)]
struct TomlConfigFile {
    config: ESConfig,
}

#[instrument(level=Level::DEBUG)]
pub async fn read_config(filename: PathBuf) -> Arc<ESConfig> {
    debug!("reading config file");

    let doc = tokio::fs::read_to_string(filename)
        .await
        .expect("failed to read config file");

    // hamfisted way to prevent sensitive info in the config file
    // from being printed to logs
    let data: TomlConfigFile = match toml::from_str(&doc) {
        Ok(val) => val,
        Err(err) => panic!("failed to parse config file: {err}"),
    };

    debug!("successfully parsed config file");
    Arc::new(data.config)
}
