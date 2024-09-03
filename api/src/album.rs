use serde::{Deserialize, Serialize};

use crate::media::MediaUuid;
use crate::message;

// structs and types

pub type AlbumUuid = i64;

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

macro_rules! album_message {
    ($s:ident) => {
        message! {$s, "album"}
    };
}

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
    pub gid: String,
    pub name: String,
    pub note: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateAlbumResp {
    pub album_uuid: AlbumUuid,
}

album_message! {CreateAlbum}

// retrieve the album properties
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetAlbumReq {
    pub album_uuid: AlbumUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetAlbumResp {
    pub album: Album,
}

album_message! {GetAlbum}

// delete an album
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeleteAlbumReq {
    pub album_uuid: AlbumUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeleteAlbumResp {}

album_message! {DeleteAlbum}

// change album properties
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateAlbumReq {
    pub album_uuid: AlbumUuid,
    pub metadata: AlbumMetadata,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateAlbumResp {}

album_message! {UpdateAlbum}

// add media to an album
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddMediaToAlbumReq {
    pub album_uuid: AlbumUuid,
    pub media_uuid: MediaUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddMediaToAlbumResp {}

album_message! {AddMediaToAlbum}

// remove media from an album
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RmMediaFromAlbumReq {
    pub album_uuid: AlbumUuid,
    pub media_uuid: MediaUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RmMediaFromAlbumResp {}

album_message! {RmMediaFromAlbum}

// search albums
//
// defaults to ""
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SearchAlbumsReq {
    pub filter: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchAlbumsResp {
    pub albums: Vec<AlbumUuid>,
}

album_message! {SearchAlbums}

// search media inside a particular album
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SearchMediaInAlbumReq {
    pub album_uuid: AlbumUuid,
    pub filter: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchMediaInAlbumResp {
    pub media: Vec<MediaUuid>,
}

album_message! {SearchMediaInAlbum}
