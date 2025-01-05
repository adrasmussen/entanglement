use serde::{Deserialize, Serialize};

use crate::endpoint;
use crate::api::media::MediaUuid;

// structs and types

pub type LibraryUuid = i64;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Library {
    pub path: String,
    pub uid: String,
    pub gid: String,
    pub count: i64,
    pub mtime: i64,
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
    pub filter: String
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
