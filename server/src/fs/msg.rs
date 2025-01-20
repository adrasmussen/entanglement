use std::collections::HashMap;

use common::api::library::{LibraryScanJob, LibraryUuid};
use crate::service::{ESM, ESMResp};

#[derive(Debug)]
pub enum FsMsg {
    Status {
        resp: ESMResp<()>
    },
    ScanLibrary {
        resp: ESMResp<()>,
        library_uuid: LibraryUuid,
    },
    ScanStatus {
        resp: ESMResp<HashMap<LibraryUuid, LibraryScanJob>>,
    },
    FixSymlinks {
        resp: ESMResp<()>,
    }
}

impl From<FsMsg> for ESM {
    fn from(value: FsMsg) -> Self {
        ESM::Fs(value)
    }
}
