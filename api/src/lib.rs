use std::collections::HashMap;

use gloo_net::http::Request;

use serde::{self, Deserialize, Serialize};

pub const URL_MATCH_IMAGES: &str = "http://localhost:8081/api/img.json";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Visibility {
    Private,
    Public,
    Hidden,
}

// the core image data struct
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Image {
    // this is filled in by the http service to match the mapping from file paths -> urls,
    // but is not actually recorded in the database
    pub url: String,
    pub file: ImageFileData,
    pub metadata: ImageMetadata,
}

// the immutable part of the image metadata, set by its physical location on the disk
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageFileData {
    pub owner: String,
    pub path: String,
    pub size: String,
    pub mtime: String,
    pub x_pixel: i32,
    pub y_pixel: i32,
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
    visibility: Option<Visibility>,
    orientation: Option<()>,
    date: Option<u64>,
    note: Option<String>,
}


// the idea here is that each change can be expressed as a "this field changed from X to Y"
pub struct ImageLogs {}

// update the image metadata, including visibility and other properties'
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageUpdateReq {
    // if the old version is not version - 1, refresh the page?
    pub version: u32,
    pub image: String,
    pub metadata: ImageMetadata,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageUpdateResp {}

pub async fn update_image() -> anyhow::Result<ImageUpdateResp> {
    todo!()
}

// this group of structs describes a request to the api server to match the images
// against some filter, and then return the result
//
// note that the response should automatically take into account the user who is
// making the request, so all of these images should be available
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageMatchReq {
    pub filter: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageMatchResp {
    pub images: HashMap<String, Image>,
}

pub async fn match_images(_filter: &ImageMatchReq) -> anyhow::Result<ImageMatchResp> {
    // when the search system is working, we can post() the filter_data and parse the response
    let match_data: ImageMatchResp = Request::get(URL_MATCH_IMAGES).send().await?.json().await?;

    Ok(match_data)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Album {
    pub owner: String,
    pub metadata: AlbumMetadata
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlbumMetadata {
    visibility: Option<Visibility>,
}

pub struct AlbumLogs {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlbumUpdateReq {
    pub album: String,
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
    pub metadata: LibraryMetadata
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LibraryMetadata {
    visibility: Option<Visibility>,
}

pub struct LibraryLogs {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LibraryUpdateReq {
    pub library: String,
    pub metadata: LibraryMetadata,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LibraryUpdateResp {}

pub async fn update_library() -> anyhow::Result<LibraryUpdateResp> {
    todo!()
}
