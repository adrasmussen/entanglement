use std::path::PathBuf;

use serde::{Serialize, Deserialize};

use crate::*;

// structs and types

pub type LibraryUuid = i64;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Library {
    pub path: PathBuf,
    pub owner: String,
    pub group: String,
    pub last_scan: String,
    pub file_count: i64,
}

// messages

// get the details for a particular library
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetLibraryReq {
    pub library_uuid: LibraryUuid,
}

pub struct GetLibaryResp {
    pub library: Library,
}

// search media inside a particular library
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchMediaInLibraryReq {
    pub library_uuid: LibraryUuid,
    pub filter: String,
    pub media_type: HashSet<MediaType>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchMediaInLibraryResp {
    pub media: Vec<MediaUuid>,
}
