use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct ESConfig {
    pub group_yaml: Option<String>,
    pub http_socket: String,
    pub mysql_url: String,
    pub media_srcdir: PathBuf,
    pub media_linkdir: PathBuf,
}
