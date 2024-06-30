use std::path::PathBuf;

use crate::service::ESMResp;

#[derive(Debug)]
pub enum FsMsg {
    _Status,
    _ScanLibrary {
        resp: ESMResp<()>,
        conn: (), // generalized db connection object
        libdir: PathBuf,
    }
}
