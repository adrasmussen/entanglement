use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// entanglement server configuration subtables
//
// mostly to keep parity with the auth/db parts, we split out
// these structs to help with the readability in config.rs
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
    // pub url_root: String,

    // location of wasm app
    pub doc_root: String,

    // pem-encoded key and cert used by the server for tls
    pub key: PathBuf,
    pub cert: PathBuf,

    // concatenated, pem-encoded ca certs to use when verifying
    // a client tls connection
    pub client_ca_cert: Option<PathBuf>,
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

    // time to wait on individual scan jobs
    pub scan_timeout: u64,
}
