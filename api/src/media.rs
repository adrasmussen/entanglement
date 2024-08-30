use serde::{Deserialize, Serialize};

use crate::album::AlbumUuid;
use crate::library::LibraryUuid;

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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MediaMessage {
    GetMedia(GetMediaReq),
    UpdateMedia(UpdateMediaReq),
    SetMediaHidden(SetMediaHiddenReq),
    SearchMedia(SearchMediaReq),
    RevSearchMediaForAlbum(RevSearchMediaForAlbumReq),
}

// fetch the media information for a particular file
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetMediaReq {
    pub media_uuid: MediaUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetMediaResp {
    pub media: Media,
}

pub async fn get_media(req: &GetMediaReq) -> anyhow::Result<GetMediaResp> {
    let resp = gloo_net::http::Request::post("/entanglement/api/media")
        .json(&MediaMessage::GetMedia(req.clone()))?
        .send()
        .await?;

    if resp.ok() {
        Ok(resp.json().await?)
    } else {
        Err(anyhow::Error::msg(resp.text().await?))
    }
}

// update the metadata
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateMediaReq {
    pub media_uuid: MediaUuid,
    pub change: MediaMetadata,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateMediaResp {}

pub async fn update_media(req: &UpdateMediaReq) -> anyhow::Result<UpdateMediaResp> {
    let resp = gloo_net::http::Request::post("/entanglement/api/media")
        .json(&MediaMessage::UpdateMedia(req.clone()))?
        .send()
        .await?;

    if resp.ok() {
        Ok(resp.json().await?)
    } else {
        Err(anyhow::Error::msg(resp.text().await?))
    }
}



// fetch the media information for a particular file
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SetMediaHiddenReq {
    pub media_uuid: MediaUuid,
    pub hidden: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SetMediaHiddenResp {}


// search media
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchMediaReq {
    pub filter: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchMediaResp {
    pub media: Vec<MediaUuid>,
}

pub async fn search_media(req: &SearchMediaReq) -> anyhow::Result<SearchMediaResp> {
    let resp = gloo_net::http::Request::post("/entanglement/api/media")
        .json(&MediaMessage::SearchMedia(req.clone()))?
        .send()
        .await?;

    if resp.ok() {
        Ok(resp.json().await?)
    } else {
        Err(anyhow::Error::msg(resp.text().await?))
    }
}

// move this to album search
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RevSearchMediaForAlbumReq {
    pub media_uuid: MediaUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RevSearchMediaForAlbumResp {
    pub albums: Vec<AlbumUuid>,
}
