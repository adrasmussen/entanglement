use serde::{Deserialize, Serialize};

use crate::*;

// structs and types

pub type AlbumUuid = u64;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Album {
    pub owner: String,
    pub group: String,
    pub metadata: AlbumMetadata,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlbumMetadata {
    pub name: Option<String>,
    pub note: Option<String>,
}

// messages

// create a new album
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateAlbumReq {
    pub album: Album,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateAlbumResp {
    pub album_uuid: AlbumUuid,
}

// delete an album
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeleteAlbumReq {
    pub album_uuid: AlbumUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeleteAlbumResp {}

// add media to an album
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddMediaToAlbumReq {
    pub album_uuid: AlbumUuid,
    pub media_uuid: MediaUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddMediaToAlbumResp {}

// remove media from an album
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RmMediaFromAlbumReq {
    pub album_uuid: AlbumUuid,
    pub media_uuid: MediaUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RmMediaFromAlbumResp {}

// retrieve the album properties
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetAlbumReq {
    pub album_uuid: AlbumUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetAlbumResp {
    pub album: Album,
}

// change album properties
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlbumUpdateReq {
    pub uuid: AlbumUuid,
    pub metadata: AlbumMetadata,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlbumUpdateResp {}

// search albums
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlbumSearchReq {
    pub filter: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlbumSearchResp {
    pub albums: Vec<AlbumUuid>,
}

// search media inside a particular album
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaSearchInAlbumReq {
    pub album_uuid: AlbumUuid,
    pub filter: String,
    pub media_type: HashSet<MediaType>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaSearchInAlbumResp {
    pub media: Vec<MediaUuid>,
}
