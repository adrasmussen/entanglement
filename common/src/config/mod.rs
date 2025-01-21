use std::{collections::HashSet, path::PathBuf};

#[derive(Clone, Debug)]
pub struct ESConfig {
    // set of groups with admin powers
    pub auth_admin_groups: HashSet<String>,

    // optional file to define groups
    pub auth_yaml_groups: Option<String>,

    // ip and port for http server
    pub http_socket: String,

    // http url root, useful for reverse proxies
    pub http_url_root: String,

    // location of wasm app
    pub http_doc_root: String,

    // user, password, host, port, and database
    pub mysql_url: String,
    pub media_srcdir: PathBuf,
    pub media_srvdir: PathBuf,
}
