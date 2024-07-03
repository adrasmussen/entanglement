use std::collections::HashMap;

use gloo_net::http::Request;

use serde::{self, Deserialize, Serialize};

pub const URL_MATCH_IMAGES: &str = "http://localhost:8081/api/img.json";

// the core image data struct
//
// this will eventually have many other fields, roughly corresponding to the EXIF
// data plus whatever else we drop on top
//
// generally these should be packaged as HashMap<String, Image>, indexed by the
// u64-as-string uuid of each image
#[derive(Clone, Serialize, Deserialize)]
pub struct Image {
    pub url: String,
}

// this group of structs describes a request to the api server to match the images
// against some filter, and then return the result
//
// note that the response should automatically take into account the user who is
// making the request, so all of these images should be available
#[derive(Clone, Serialize, Deserialize)]
pub struct ImageMatchReq {
    pub filter: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ImageMatchResp {
    pub images: HashMap<String, Image>,
}

pub async fn match_images(_filter: &ImageMatchReq) -> anyhow::Result<ImageMatchResp> {
    // when the search system is working, we can post() the filter_data and parse the response
    let match_data: ImageMatchResp = Request::get(URL_MATCH_IMAGES).send().await?.json().await?;

    Ok(match_data)
}

#[derive(Debug)]
pub enum ImageVisibility {
    Private,
    Public,
    Hidden,
}
