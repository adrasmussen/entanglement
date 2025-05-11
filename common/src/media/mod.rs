use std::path::{Path, PathBuf};

use anyhow::Result;
use hex::encode;
use sha2::{Digest, Sha512};
use tokio::{
    fs::File,
    io::{AsyncReadExt, BufReader},
};

use api::media::MediaMetadata;
use image::create_image_thumbnail;
use video::create_video_thumbnail;

pub mod image;
pub mod video;

// intermediate struct used by media processing functions
#[derive(Clone, Debug)]
pub struct MediaData {
    pub hash: String,
    pub date: String,
    pub metadata: MediaMetadata,
}

pub async fn content_hash(path: &PathBuf) -> Result<String> {
    let file = File::open(&path).await?;

    let mut hasher = Sha512::new();
    let mut buffer = [0; 8192];

    // TODO -- perf tuning
    let mut reader = BufReader::with_capacity(8182, file);

    while reader.read(&mut buffer).await? > 0 {
        hasher.update(buffer);
    }

    Ok(encode(hasher.finalize()))
}

pub async fn create_thumbnail(path: &PathBuf, thumbnail_path: &PathBuf, scratch_dir: &Path, metadata: &MediaMetadata) -> Result<()> {
    match metadata {
        MediaMetadata::Image => {
            Box::pin(create_image_thumbnail(path, thumbnail_path)).await?
        }
        MediaMetadata::Video => {
            Box::pin(create_video_thumbnail(
                path,
                thumbnail_path,
                scratch_dir,
            ))
            .await?
        }
        _ => return Err(anyhow::Error::msg("no thumbnail method found")),
    };

    Ok(())
}
