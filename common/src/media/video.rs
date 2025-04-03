use std::path::PathBuf;

use anyhow::Result;
use tokio::process::Command;
use tracing::{info, debug, instrument};

use crate::media::{
    image::{create_image_thumbnail, hash_image},
    MediaData,
};
use api::media::MediaMetadata;

#[instrument(skip_all)]
pub async fn process_video(path: &PathBuf, scratchdir: &PathBuf) -> Result<MediaData> {
    debug!("starting processing video");
    let mut img_path = scratchdir.clone();
    img_path.push("hash.png");

    create_video_ffmpeg_image(&path, &img_path).await?;

    let hash = hash_image(&img_path)?;

    Ok(MediaData {
        hash: hash,
        date: "".to_owned(),
        metadata: MediaMetadata::Video,
    })
}

#[instrument(skip_all)]
pub async fn create_video_thumbnail(
    original_path: PathBuf,
    thumbnail_path: PathBuf,
    scratchdir: &PathBuf,
) -> Result<()> {
    debug!("started creating video thumbnail");

    // we could reuse the hash image, but it's bad form to depend on side effects
    let mut img_path = scratchdir.clone();
    img_path.push("thumb.png");

    create_video_ffmpeg_image(&original_path, &img_path).await?;
    create_image_thumbnail(img_path, thumbnail_path)?;

    Ok(())
}

#[instrument]
async fn create_video_ffmpeg_image(original_path: &PathBuf, img_path: &PathBuf) -> Result<()> {
    let handle = Command::new("ffmpegthumbnailer")
        .args(["-s", "0"])
        .args(["-i", &original_path.to_string_lossy()])
        .args(["-o", &img_path.to_string_lossy()])
        .kill_on_drop(true)
        .output().await?;

    if !handle.status.success() {
        return Err(anyhow::Error::msg("ffmegthumbnailer failed to process the image"))
    }

    Ok(())
}
