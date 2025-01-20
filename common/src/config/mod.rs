use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct ESConfig {
    // optional file to define groups
    pub group_yaml: Option<String>,

    // ip and port for http server
    pub http_socket: String,

    // http url root, useful for reverse proxies
    pub http_url_root: String,

    // location of wasm app
    pub http_doc_root: String,

    // user, password, host, port, and database
    pub mysql_url: String,
    pub media_srcdir: PathBuf,
    pub media_linkdir: PathBuf,
}
