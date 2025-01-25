use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::endpoint;
use crate::media::MediaUuid;

// structs and types

pub type LibraryUuid = i64;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Library {
    pub path: String,
    pub gid: String,
    pub mtime: i64,
    pub count: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LibraryUpdate {
    pub count: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LibraryScanJob {
    pub start_time: i64,
    pub file_count: i64,
    pub error_count: i64,
    pub status: String, // placeholder for a better type
}

// messages

// get the details for a particular library
endpoint!(GetLibrary);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetLibraryReq {
    pub library_uuid: LibraryUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetLibraryResp {
    pub library: Library,
}

// find libraries
endpoint!(SearchLibraries);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchLibrariesReq {
    pub filter: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchLibrariesResp {
    pub libraries: Vec<LibraryUuid>,
}

// find media inside of a library
endpoint!(SearchMediaInLibrary);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchMediaInLibraryReq {
    pub library_uuid: LibraryUuid,
    pub filter: String,
    pub hidden: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchMediaInLibraryResp {
    pub media: Vec<MediaUuid>,
}

// start a scan on a library
endpoint!(StartLibraryScan);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StartLibraryScanReq {
    pub library_uuid: LibraryUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StartLibraryScanResp {}

// get status of the library scanner engine
endpoint!(GetLibraryScan);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetLibraryScanReq {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetLibraryScanResp {
    pub jobs: HashMap<LibraryUuid, LibraryScanJob>,
}

// get status of the library scanner engine
endpoint!(StopLibraryScan);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StopLibraryScanReq {
    pub library_uuid: LibraryUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StopLibraryScanResp {}
