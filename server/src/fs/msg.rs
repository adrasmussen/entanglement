use std::collections::HashMap;

use crate::service::{ESMResp, ESM};
use common::api::library::{LibraryScanJob, LibraryUuid};

#[derive(Debug)]
pub enum FsMsg {
    Status {
        resp: ESMResp<()>,
    },
    ScanLibrary {
        resp: ESMResp<()>,
        library_uuid: LibraryUuid,
    },
    ScanStatus {
        resp: ESMResp<HashMap<LibraryUuid, LibraryScanJob>>,
    },
    StopScan {
        resp: ESMResp<()>,
        library_uuid: LibraryUuid,
    },
    FixSymlinks {
        resp: ESMResp<()>,
    },
}

impl From<FsMsg> for ESM {
    fn from(value: FsMsg) -> Self {
        ESM::Fs(value)
    }
}
