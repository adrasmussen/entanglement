use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::album::AlbumUuid;
use crate::library::LibraryUuid;

// structs and types

pub type MediaUuid = i64;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Media {
    pub library_uuid: LibraryUuid,
    pub path: PathBuf,
    pub hidden: bool,
    pub metadata: MediaMetadata,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaMetadata {
    pub date: String,
    pub note: String,
}

// messages

// fetch the media information for a particular file
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetMediaReq {
    pub media_uuid: MediaUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetMediaResp {
    pub media: Media,
}

// update the metadata
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateMediaReq {
    pub media_uuid: MediaUuid,
    pub change: MediaMetadata,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateMediaResp {}

// fetch the media information for a particular file
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SetMediaHiddenReq {
    pub media_uuid: MediaUuid,
    pub hidden: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SetMediaHiddenResp {}


// search media, optionally with a filter on type
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchMediaReq {
    pub filter: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchMediaResp {
    pub media: Vec<MediaUuid>,
}

// reverse search and find all albums that contain
// a particular media file
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RevSearchMediaForAlbumReq {
    pub media_uuid: MediaUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RevSearchMediaForAlbumResp {
    pub albums: Vec<AlbumUuid>,
}
