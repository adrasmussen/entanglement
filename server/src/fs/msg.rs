use api::library::{LibraryScanResult, LibraryUuid};

use crate::service::{ESM, ESMResp};

#[derive(Debug)]
pub enum FsMsg {
    Status {
        resp: ESMResp<()>
    },
    ScanLibrary {
        resp: ESMResp<LibraryScanResult>,
        library_uuid: LibraryUuid,
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
