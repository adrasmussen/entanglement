use serde::{Deserialize, Serialize};

use crate::media::MediaUuid;

// structs and types

pub type AlbumUuid = u64;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Album {
    pub uid: String,
    pub gid: String,
    pub metadata: AlbumMetadata,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlbumMetadata {
    pub name: String,
    pub note: String,
}

// messages

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AlbumMessage {
    CreateAlbum(CreateAlbumReq),
    GetAlbum(GetAlbumReq),
    DeleteAlbum(DeleteAlbumReq),
    UpdateAlbum(UpdateAlbumReq),
    AddMediaToAlbum(AddMediaToAlbumReq),
    RmMediaFromAlbum(RmMediaFromAlbumReq),
    SearchAlbums(SearchAlbumsReq),
    SearchMediaInAlbum(SearchMediaInAlbumReq),
}

// create a new album
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateAlbumReq {
    pub album: Album,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateAlbumResp {
    pub album_uuid: AlbumUuid,
}

// retrieve the album properties
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetAlbumReq {
    pub album_uuid: AlbumUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetAlbumResp {
    pub album: Album,
}

// delete an album
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeleteAlbumReq {
    pub album_uuid: AlbumUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeleteAlbumResp {}

// change album properties
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateAlbumReq {
    pub album_uuid: AlbumUuid,
    pub metadata: AlbumMetadata,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateAlbumResp {}

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

// search albums
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchAlbumsReq {
    pub filter: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchAlbumsResp {
    pub albums: Vec<AlbumUuid>,
}

// search media inside a particular album
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchMediaInAlbumReq {
    pub album_uuid: AlbumUuid,
    pub filter: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchMediaInAlbumResp {
    pub media: Vec<MediaUuid>,
}
