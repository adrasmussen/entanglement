use serde::{Deserialize, Serialize};

use crate::api::{album::AlbumUuid, library::LibraryUuid, comment::CommentUuid};
use crate::endpoint;

// structs and types

pub type MediaUuid = i64;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MediaMetadata {
    Image {
        date: String,
    },
    Video {
        length: i64,
        date: String,
    },
    VideoSlice{
        start: i64,
        end: i64,
        date: String,
    },
    Audio,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Media {
    pub library_uuid: LibraryUuid,
    pub path: String,
    pub hash: String,
    pub hidden: bool,
    pub attention: bool,
    pub mtime: i64,
    pub metadata: MediaMetadata,
}

// messages

// fetch the media information for a particular file
endpoint!(GetMedia);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetMediaReq {
    pub media_uuid: MediaUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetMediaResp {
    pub media: Media,
    pub albums: Vec<AlbumUuid>,
    pub comments: Vec<CommentUuid>,
}

// update the metadata
endpoint!(UpdateMedia);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateMediaReq {
    pub media_uuid: MediaUuid,
    pub hidden: Option<bool>,
    pub attention: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateMediaResp {}

// search media
//
// note that we can implement a more complicated
// filter struct later
endpoint!(SearchMedia);

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SearchMediaReq {
    pub filter: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SearchMediaResp {
    pub media: Vec<MediaUuid>,
}
