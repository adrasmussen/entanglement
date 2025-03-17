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

    // ip and port for http server
    pub http_socket: String,

    // http url root, useful for reverse proxies
    pub http_url_root: String,

    // location of wasm app
    pub http_doc_root: String,

    // user, password, host, port, and database
    pub mariadb_url: String,

    // read-only source path where media can be located
    //
    // libraries should be subfolders of this path
    pub media_srcdir: PathBuf,

    // read-write path where symlinks are created, as
    // well as subfolders for thumbnails and slices
    pub media_srvdir: PathBuf,

    // maximum number of tokio tasks use for running scan jobs,
    // which should be less than the number of OS threads since
    // some of the crates have blocking io calls
    pub fs_scanner_threads: usize,
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
