use serde::{Deserialize, Serialize};

use crate::{http_endpoint, media::MediaUuid, search::SearchFilter};

// structs and types

pub type LibraryUuid = u64;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Library {
    // the path to the library, relative to the media_srcdir
    pub path: String,
    // effective user for running scripts
    pub uid: String,
    // owner gid used to check privileges
    pub gid: String,
    // last modification time of the library
    pub mtime: u64,
    // number of files seen on the last count
    pub count: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LibraryUpdate {
    pub count: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LibraryScanJob {
    pub start_time: i64,
    pub file_count: i64,
    pub error_count: i64,
    pub status: String, // placeholder for a better type
}

// messages

// get the details for a particular library
http_endpoint!(GetLibrary);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GetLibraryReq {
    pub library_uuid: LibraryUuid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GetLibraryResp {
    pub library: Library,
}

// find libraries
http_endpoint!(SearchLibraries);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchLibrariesReq {
    pub filter: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchLibrariesResp {
    pub libraries: Vec<LibraryUuid>,
}

// find media inside of a library
http_endpoint!(SearchMediaInLibrary);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchMediaInLibraryReq {
    pub library_uuid: LibraryUuid,
    pub hidden: Option<bool>,
    pub filter: SearchFilter,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchMediaInLibraryResp {
    pub media: Vec<MediaUuid>,
}
