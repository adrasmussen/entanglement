use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{
    collection::CollectionUuid, comment::CommentUuid, endpoint, library::LibraryUuid,
    search::SearchFilter,
};

// structs and types

pub type MediaUuid = i64;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum MediaMetadata {
    Image,
    Video,
    VideoSlice,
    Audio,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Media {
    pub library_uuid: LibraryUuid,
    pub path: String,
    pub size: u64,
    pub chash: String,
    pub phash: String,
    pub mtime: i64,
    pub hidden: bool,
    pub date: String,
    pub note: String,
    pub tags: HashSet<String>,
    pub metadata: MediaMetadata,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaUpdate {
    pub hidden: Option<bool>,
    pub date: Option<String>,
    pub note: Option<String>,
    pub tags: Option<HashSet<String>>,
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
    pub collections: Vec<CollectionUuid>,
    pub comments: Vec<CommentUuid>,
}

// update the metadata
endpoint!(UpdateMedia);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateMediaReq {
    pub media_uuid: MediaUuid,
    pub update: MediaUpdate,
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
    pub filter: SearchFilter,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SearchMediaResp {
    pub media: Vec<MediaUuid>,
}

// find similar media
endpoint!(SimilarMedia);

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SimilarMediaReq {
    pub media_uuid: MediaUuid,
    pub distance: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SimilarMediaResp {
    pub media: Vec<MediaUuid>,
}
