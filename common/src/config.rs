use std::{path::PathBuf, sync::Arc};

use serde::{Deserialize, Serialize};
use tokio;
use toml;
use tracing::{debug, instrument, Level};

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
    pub mariadb: Option<MariaDbConfig>,
    pub tomlfile: Option<TomlFileConfig>,
    pub proxyheader: Option<ProxyHeaderConfig>,
}

// authentication config options

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum AuthnBackend {
    // header set by reverse proxy
    ProxyHeader,
    // a toml file with usernames and passwords
    TomlFile,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProxyHeaderConfig {
    pub header: String,
}

// authorization config options

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum AuthzBackend {
    // a toml file with group memberships
    TomlFile,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TomlFileConfig {
    pub filename: PathBuf,
}

// database config options

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum DbBackend {
    MariaDB,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MariaDbConfig {
    pub url: String,
}

// core service config options

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FsConfig {
    // read-only source path where media can be located
    //
    // libraries should be subfolders of this path
    pub media_srcdir: PathBuf,

    // read-write path where symlinks are created, as
    // well as subfolders for thumbnails and slices
    pub media_srvdir: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HttpConfig {
    // ip and port for http server
    pub socket: String,

    // http url root, since we should be running behind a
    // reverse proxy
    //
    // currently set at compile time, see api lib.rs
    //pub url_root: String,

    // location of wasm app
    pub doc_root: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TaskConfig {
    // maximum number of tokio tasks use for running scan jobs,
    // which should be less than the number of OS threads since
    // some of the crates have blocking io calls
    pub scan_threads: usize,

    // temporary folder used by scanner for things like creating
    // video thumbnails
    pub scan_scratch: PathBuf,
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

    let data: TomlConfigFile = toml::from_str(&doc).expect("failed to parse config file");

    debug!("successfully parsed config file");
    Arc::new(data.config)
}
