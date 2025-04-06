use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio;
use toml;
use tracing::{debug, instrument, Level};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ESConfig {
    // header set by reverse proxy
    pub authn_proxy_header: Option<String>,

    // a toml file with usernames and passwords
    pub authn_toml_file: Option<String>,

    // set of groups with admin powers
    pub authz_admin_groups: Option<HashSet<String>>,

    // a toml file with group memberships
    pub authz_toml_file: Option<String>,

    pub http: HttpConfig,

    pub fs: FsConfig,

    // ip and port for http server
    pub http_socket: String,

    // user, password, host, port, and database
    pub mariadb_url: String,

    // maximum number of tokio tasks use for running scan jobs,
    // which should be less than the number of OS threads since
    // some of the crates have blocking io calls
    pub scan_threads: usize,

    // temporary folder used by scanner for things like creating
    // video thumbnails
    pub scan_scratch: PathBuf,
}

enum Authn {}

#[derive(Clone, Debug, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FsConfig {
    // read-only source path where media can be located
    //
    // libraries should be subfolders of this path
    pub media_srcdir: PathBuf,

    // read-write path where symlinks are created, as
    // well as subfolders for thumbnails and slices
    pub media_srvdir: PathBuf,
}

// in order to extract the config table from a larger document, we need to specify it
// as a subtable of the root node, i.e. a substruct
#[derive(Debug, Serialize, Deserialize)]
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
