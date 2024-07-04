use std::path::PathBuf;

use crate::service::ESMResp;

#[derive(Debug)]
pub enum FsMsg {
    Status {
        resp: ESMResp<()>
    },
    ScanLibrary {
        resp: ESMResp<()>,
        library: String,
    },
    RescanFile {
        resp: ESMResp<()>,
        file: PathBuf,
    }
}
