use serde::{Deserialize, Serialize};

use crate::media::MediaUuid;

// structs and types

pub type LibraryUuid = i64;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Library {
    pub path: String,
    pub gid: String,
    pub metadata: LibraryMetadata,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LibraryMetadata {
    pub file_count: i64,
    pub last_scan: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LibraryScanResult {
    pub count: i64,
    pub errors: Vec<String>,
}

// messages

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LibraryMessage {
    AddLibrary(AddLibraryReq),
    GetLibary(GetLibraryReq),
    SearchMediaInLibrary(SearchMediaInLibraryReq),
    ScanLibrary(ScanLibraryReq),
}

// attach a library to the database
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddLibraryReq {
    pub library: Library,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddLibaryResp {
    pub library_uuid: LibraryUuid,
}

// get the details for a particular library
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetLibraryReq {
    pub library_uuid: LibraryUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetLibaryResp {
    pub library: Library,
}

// search media inside a particular library
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScanLibraryReq {
    pub library_uuid: LibraryUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScanLibraryResp {
    pub result: LibraryScanResult,
}
