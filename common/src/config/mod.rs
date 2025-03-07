use std::{collections::HashSet, path::PathBuf};

#[derive(Clone, Debug)]
pub struct ESConfig {
    // header set by reverse proxy
    pub authn_proxy_header: Option<String>,

    // set of groups with admin powers
    pub authz_admin_groups: HashSet<String>,

    // file to define group membership, but not passwords
    pub authz_yaml_groups: Option<String>,

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
