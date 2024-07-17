use std::path::PathBuf;

use crate::service::ESMResp;

#[derive(Debug)]
pub enum FsMsg {
    _Status {
        resp: ESMResp<()>
    },
    _ScanLibrary {
        resp: ESMResp<()>,
        library: String,
    },
    _RescanFile {
        resp: ESMResp<()>,
        file: PathBuf,
    },
    _FixSymlinks {
        resp: ESMResp<()>,
    }
}
