use std::collections::{HashMap, HashSet};

use gloo_net::http::Request;

use serde::{self, Deserialize, Serialize};

pub const URL_MATCH_IMAGES: &str = "http://localhost:8081/api/img.json";

pub type ImageUuid = u64;
pub type AlbumUuid = u64;
pub type LibraryUuid = u64;

// the core image data struct
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Image {
    pub data: ImageData,
    pub metadata: ImageMetadata,
}

// the immutable part of the image metadata, set by the file itself
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageData {
    pub owner: String,
    pub path: String,
    pub datetime_original: i64, // Unix timestamp for original digital photos
    pub x_pixel: u32,
    pub y_pixel: u32,
}

// the mutable part of the image metadata
//
// this struct pulls double duty as the input and output from the database, meaning that
// the semantics around the Option are a bit strange
//
// specifically, None only has meaning when updating the metadata for an image  -- the
// database columns should be NOT NULL and thus any read or initial write should be Some()
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageMetadata {
    pub orientation: Option<u32>,
    pub date: Option<String>, // eventually convert to PartialDate
    pub note: Option<String>,
}

// update the image metadata, including visibility and other properties'
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageUpdateReq {
    pub uuid: ImageUuid,
    pub metadata: ImageMetadata,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageUpdateResp {}

pub async fn update_image() -> anyhow::Result<ImageUpdateResp> {
    todo!()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageSearchReq {
    pub filter: String, // we concatenate the fields on the search so it's a single string
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageSearchResp {
    pub images: HashMap<ImageUuid, Image>,
}

pub async fn search_images(req: &ImageSearchReq) -> anyhow::Result<ImageSearchResp> {
    let resp: ImageSearchResp = Request::post("/api/search/image").json(req)?.send().await?.json().await?;
    Ok(resp)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Album {
    pub owner: String,
    pub group: String,
    pub metadata: AlbumMetadata,
    pub images: Vec<ImageUuid>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlbumMetadata {
    pub name: Option<String>,
    pub note: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlbumUpdateReq {
    pub uuid: AlbumUuid,
    pub metadata: AlbumMetadata,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlbumUpdateResp {}

pub async fn update_album() -> anyhow::Result<AlbumUpdateResp> {
    todo!()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Library {
    pub owner: String,
    pub metadata: LibraryMetadata,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LibraryMetadata {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LibraryUpdateReq {
    pub uuid: LibraryUuid,
    pub metadata: LibraryMetadata,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LibraryUpdateResp {}

pub async fn update_library() -> anyhow::Result<LibraryUpdateResp> {
    todo!()
}
