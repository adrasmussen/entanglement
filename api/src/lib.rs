use std::collections::HashMap;

use gloo_net::http::Request;

use serde::{self, Deserialize, Serialize};

pub const URL_MATCH_IMAGES: &str = "http://localhost:8081/api/img.json";

#[derive(Clone, Serialize, Deserialize)]
pub struct ImageFilter {
    pub filter: String,
}

// the key is the SHA256 of the image
#[derive(Clone, Serialize, Deserialize)]
pub struct MatchedImages {
    pub images: HashMap<String, Image>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Image {
    pub url: String,
}

// each of these functions describes part of the api contract
pub async fn match_images(_filter: &ImageFilter) -> anyhow::Result<MatchedImages> {
    // when the search system is working, we can post() the filter_data and parse the response
    let match_data: MatchedImages = Request::get(URL_MATCH_IMAGES).send().await?.json().await?;

    Ok(match_data)
}