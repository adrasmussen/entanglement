use serde::{Deserialize, Serialize};

use crate::api::media::MediaUuid;
use crate::endpoint;

// structs and types

pub type AlbumUuid = i64;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Album {
    pub uid: String,
    pub gid: String,
    pub mtime: i64,
    pub name: String,
    pub note: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlbumUpdate {
    pub name: Option<String>,
    pub note: Option<String>,
}

// messages

// create a new album
endpoint!(AddAlbum);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddAlbumReq {
    pub gid: String,
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddAlbumResp {
    pub album_uuid: AlbumUuid,
}

// get details on an album
//
// note that we fetch the media with
// a blank filter in another call
endpoint!(GetAlbum);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetAlbumReq {
    pub album_uuid: AlbumUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetAlbumResp {
    pub album: Album,
}

// delete an album
endpoint!(DeleteAlbum);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeleteAlbumReq {
    pub album_uuid: AlbumUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeleteAlbumResp {}

// change album properties
endpoint!(UpdateAlbum);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateAlbumReq {
    pub album_uuid: AlbumUuid,
    pub update: AlbumUpdate,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateAlbumResp {}


// add media to an album
endpoint!(AddMediaToAlbum);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddMediaToAlbumReq {
    pub album_uuid: AlbumUuid,
    pub media_uuid: MediaUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddMediaToAlbumResp {}

// remove media from an album
endpoint!(RmMediaFromAlbum);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RmMediaFromAlbumReq {
    pub album_uuid: AlbumUuid,
    pub media_uuid: MediaUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RmMediaFromAlbumResp {}

// search albums
//
// defaults to ""
endpoint!(SearchAlbums);

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SearchAlbumsReq {
    pub filter: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchAlbumsResp {
    pub albums: Vec<AlbumUuid>,
}

// search media inside a particular album
endpoint!(SearchMediaInAlbum);

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SearchMediaInAlbumReq {
    pub album_uuid: AlbumUuid,
    pub filter: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchMediaInAlbumResp {
    pub media: Vec<MediaUuid>,
}
