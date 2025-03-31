use std::path::PathBuf;

use anyhow::Result;
use blockhash::blockhash256;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader};
use tracing::{debug, info_span, instrument, Level};

use crate::media::MediaData;
use api::media::MediaMetadata;

#[instrument]
pub async fn process_video(path: &PathBuf) -> Result<MediaData> {



    Ok(MediaData {
        hash: "placeholder".to_owned(),
        date: "placeholder".to_owned(),
        metadata: MediaMetadata::Video,
    })
}

pub fn create_video_thumbnail(original_path: PathBuf, thumbnail_path: PathBuf) -> Result<()> {
    let span = info_span!("create_video_thumbnail", original_path=?original_path);
    let _ = span.enter();

    todo!()
}
