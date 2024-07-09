use std::collections::HashMap;

use gloo_net::http::Request;

use serde::{self, Deserialize, Serialize};

pub const URL_MATCH_IMAGES: &str = "http://localhost:8081/api/img.json";

pub type ImageUuid = u64;
pub type AlbumUuid = u64;
pub type LibraryUuid = u64;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Visibility {
    Private,
    Public,
    Hidden,
}

impl From<String> for Visibility {
    fn from(string: String) -> Visibility {
        match string.as_str() {
            "Public" | "public" => Visibility::Public,
            "Hidden" | "hidden" => Visibility::Hidden,
            _ => Visibility::Private,
        }
    }
}

impl Into<String> for Visibility {
    fn into(self) -> String {
        match self {
            Visibility::Private => String::from("Private"),
            Visibility::Public => String::from("Public"),
            Visibility::Hidden => String::from("Hidden"),
        }
    }
}

// the core image data struct
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Image {
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
    pub visibility: Option<Visibility>,
    pub orientation: Option<i8>,
    pub date: Option<u64>,
    pub note: Option<String>,
}

// the idea here is that each change can be expressed as a "this field changed from X to Y"
pub struct ImageLog {
    pub uuid: ImageUuid,
    pub date: u64,
    pub user: String,
    pub log: String,
}

// update the image metadata, including visibility and other properties'
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageUpdateReq {
    // if the old version is not version - 1, refresh the page?
    pub version: i32,
    pub uuid: ImageUuid,
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

// we don't allow unrestricted SQL queries, but instead use a structured search that
// can set each of these
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageFilterFragment {
    pub field: ImageFilterField,
    pub search: ImageFilterSearch,
    pub join: ImageFilterJoin,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ImageFilterField {
    Owner,
    Visibility,
    Year,
    Note,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ImageFilterSearch {
    Contains,
    Exact,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ImageFilterJoin {
    And,
    Or,
}

// the hash key is the index of the search fragment, since order matters
// when computing AND/OR of the the WHERE clauses
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageFilter {
    pub filter: HashMap<i32, ImageFilterFragment>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilterImageReq {
    pub filter: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilterImageResp {
    pub images: HashMap<ImageUuid, Image>,
}

pub async fn filter_images(_filter: &FilterImageReq) -> anyhow::Result<FilterImageResp> {
    // when the search system is working, we can post() the filter_data and parse the response
    let match_data: FilterImageResp = Request::get(URL_MATCH_IMAGES).send().await?.json().await?;

    Ok(match_data)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Album {
    pub owner: String,
    pub metadata: AlbumMetadata,
    pub images: Vec<ImageUuid>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlbumMetadata {
    pub visibility: Option<Visibility>,
}

pub struct AlbumLogs {}

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
pub struct LibraryMetadata {
    visibility: Option<Visibility>,
}

pub struct LibraryLogs {}

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
