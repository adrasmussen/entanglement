use api::library::*;

use crate::service::ESMResp;
use crate::fs::scan::ScanReport;

#[derive(Debug)]
pub enum FsMsg {
    Status {
        resp: ESMResp<()>
    },
    ScanLibrary {
        resp: ESMResp<ScanReport>,
        library_uuid: LibraryUuid,
    },
    FixSymlinks {
        resp: ESMResp<()>,
    }
}
