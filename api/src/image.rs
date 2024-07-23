use serde::{Deserialize, Serialize};

use crate::*;

// structs and types

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct Image {
    pub datetime_original: i64, // Unix timestamp for original digital photos
    pub orientation: u32,
    pub x_pixel: u32,
    pub y_pixel: u32,
}

// messages

// update the image metadata
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageUpdateReq {
    pub uuid: MediaUuid,
    pub metadata: Image,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageUpdateResp {}
