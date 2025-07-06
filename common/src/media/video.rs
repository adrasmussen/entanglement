use std::path::{Path, PathBuf};

use anyhow::Result;
use tokio::process::Command;
use tracing::{debug, instrument};

use crate::media::{
    MediaData,
    image::{create_image_thumbnail, hash_image},
};
use api::media::MediaMetadata;

// video calculations

#[instrument(skip_all)]
pub async fn process_video(path: &PathBuf, scratchdir: &Path) -> Result<MediaData> {
    debug!("starting processing video");
    let mut img_path = scratchdir.to_path_buf();
    img_path.push("hash.png");

    create_video_ffmpeg_image(path, &img_path).await?;

    let date = parse_video_metadata_dump(path).await?;

    let hash = hash_image(&img_path).await?;

    Ok(MediaData {
        hash,
        date,
        metadata: MediaMetadata::Video,
    })
}

#[instrument(skip_all)]
pub async fn create_video_thumbnail(
    original_path: &PathBuf,
    thumbnail_path: &PathBuf,
    scratchdir: &Path,
) -> Result<()> {
    debug!("started creating video thumbnail");

    // we could reuse the hash image, but it's bad form to depend on side effects
    let mut img_path = scratchdir.to_path_buf();
    img_path.push("thumb.png");

    create_video_ffmpeg_image(original_path, &img_path).await?;
    create_image_thumbnail(&img_path, thumbnail_path).await?;

    Ok(())
}

#[instrument]
async fn create_video_ffmpeg_image(original_path: &PathBuf, img_path: &PathBuf) -> Result<()> {
    let handle = Command::new("ffmpegthumbnailer")
        .args(["-s", "0"])
        .args(["-i", &original_path.to_string_lossy()])
        .args(["-o", &img_path.to_string_lossy()])
        .kill_on_drop(true)
        .output()
        .await?;

    if !handle.status.success() {
        return Err(anyhow::Error::msg(
            "ffmegthumbnailer failed to process the image",
        ));
    }

    Ok(())
}

#[instrument]
async fn parse_video_metadata_dump(original_path: &PathBuf) -> Result<String> {
    let handle = Command::new("ffprobe")
        .args(["-v", "quiet"])
        .args(["-select_streams", "v:0"])
        .args(["-show_entries", "stream_tags=creation_time"])
        .args(["-output_format", "default=noprint_wrappers=1:nokey=1"])
        .arg(original_path)
        .kill_on_drop(true)
        .output()
        .await?;

    if !handle.status.success() {
        return Err(anyhow::Error::msg("ffprobe failed to process the media"));
    }

    // this is likely to come out as an rfc3339-formatted string, so do some
    // minimal parsing to make it easily searchable
    let out = String::from_utf8(handle.stdout)?;

    let out = out.replace("T", " ");

    if let Some(v) = out.split_once(".") {
        return Ok(v.0.to_string());
    } else {
        return Ok(out.to_string());
    }
}
