use serde::{Deserialize, Serialize};

use crate::message;
use crate::{library::LibraryUuid, ticket::TicketUuid, album::AlbumUuid};

// structs and types

pub type MediaUuid = i64;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Media {
    pub library_uuid: LibraryUuid,
    pub path: String,
    pub hidden: bool,
    pub metadata: MediaMetadata,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaMetadata {
    pub date: String,
    pub note: String,
}

// messages

macro_rules! media_message {
    ($s:ident) => {
        message! {$s, "media"}
    };
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MediaMessage {
    GetMedia(GetMediaReq),
    UpdateMedia(UpdateMediaReq),
    SetMediaHidden(SetMediaHiddenReq),
    SearchMedia(SearchMediaReq),
}

// fetch the media information for a particular file
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetMediaReq {
    pub media_uuid: MediaUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetMediaResp {
    pub media: Media,
    pub albums: Vec<AlbumUuid>,
    pub tickets: Vec<TicketUuid>,
}

media_message! {GetMedia}

// update the metadata
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateMediaReq {
    pub media_uuid: MediaUuid,
    pub change: MediaMetadata,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateMediaResp {}

media_message! {UpdateMedia}

// fetch the media information for a particular file
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SetMediaHiddenReq {
    pub media_uuid: MediaUuid,
    pub hidden: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SetMediaHiddenResp {}

media_message! {SetMediaHidden}

// search media
//
// defaults to ""
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SearchMediaReq {
    pub filter: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SearchMediaResp {
    pub media: Vec<MediaUuid>,
}

media_message! {SearchMedia}
