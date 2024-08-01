use std::collections::HashSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub mod album;
pub mod user;
pub mod image;
pub mod library;
pub mod ticket;
pub mod group;

use album::AlbumUuid;
use library::LibraryUuid;

// pub async fn search_images(req: &ImageSearchReq) -> anyhow::Result<ImageSearchResp> {
//     let resp: ImageSearchResp = Request::post("/api/search/image")
//         .json(req)?
//         .send()
//         .await?
//         .json()
//         .await?;
//     Ok(resp)
// }


// structs and types

pub type MediaUuid = i64;

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum MediaType {
    Image(crate::image::Image),
    Video,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Media {
    pub media_type: MediaType,
    pub library: LibraryUuid,
    pub path: PathBuf,
    pub metadata: MediaMetadata,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaMetadata {
    pub hidden: bool,
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

// search media, optionally with a filter on type
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaSearchReq {
    pub filter: String,
    pub media_type: HashSet<MediaType>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaSearchResp {
    pub media: Vec<MediaUuid>,
}

// reverse search and find all albums that contain
// a particular media file
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaRevSearchForAlbumReq {
    pub media_uuid: MediaUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaRevSearchForAlbumResp {
    pub albums: Vec<AlbumUuid>,
}
