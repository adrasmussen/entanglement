use std::path::PathBuf;

use anyhow::Result;
use blockhash::blockhash256;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader};
use tokio::task::spawn_blocking;
use tracing::{debug, instrument};

use crate::media::MediaData;
use api::media::MediaMetadata;

// image calculations
//
// unfortunately, the image crate is largely built from synchronous std::io tech, which
// means spawn_blocking() wrappers on all of it to avoid jamming the runtime

#[instrument(skip_all)]
pub async fn hash_image(path: &PathBuf) -> Result<String> {
    debug!("calculating hash");

    let path = path.clone();

    let hash = spawn_blocking(move || {
        let image = image::open(path)?;

        Result::<String>::Ok(blockhash256(&image).to_string())
    })
    .await??;

    Ok(hash)
}

#[instrument(skip_all)]
pub async fn process_image(path: &PathBuf) -> Result<MediaData> {
    debug!("processing image");

    let path = path.clone();

    // exif processing
    //
    // we attempt to read the exif metadata for the image to extract the date
    // following the exif docs, open the file synchronously and read from the container
    let (path, datetime_original) = spawn_blocking(move || {
        let file = std::fs::File::open(&path)?;

        let mut bufreader = std::io::BufReader::new(file);

        let exifreader = exif::Reader::new();

        let datetime_original = match exifreader.read_from_container(&mut bufreader).ok() {
            None => String::from(""),
            Some(exif) => exif
                .get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
                .map(|dto| format!("{}", dto.display_value()))
                .unwrap_or_default(),
        };

        Result::<(PathBuf, String)>::Ok((path, datetime_original))
    })
    .await??;

    let hash = hash_image(&path).await?;

    debug!("finshed processing image");

    Ok(MediaData {
        hash: hash,
        date: datetime_original,
        metadata: MediaMetadata::Image,
    })
}

#[instrument]
pub async fn create_image_thumbnail(
    original_path: &PathBuf,
    thumbnail_path: &PathBuf,
) -> Result<()> {
    debug!("creating thumbnail");

    let original_path = original_path.clone();
    let thumbnail_path = thumbnail_path.clone();

    spawn_blocking(move || {
        let mut decoder = ImageReader::open(original_path.clone())?.into_decoder()?;

        // this both solves the crate version collision and corrects the orientation, too
        let orientation = decoder.orientation()?;

        debug!({orientation = ?orientation}, "orientation for image");

        let image = DynamicImage::from_decoder(decoder)?;

        // create the thumbnail with bounds, not exact sizing
        let mut thumbnail = image.thumbnail(400, 400);

        thumbnail.apply_orientation(orientation);

        thumbnail.save_with_format(thumbnail_path, ImageFormat::Png)?;

        debug!("finished creating thumbnail");

        Ok(())
    })
    .await?
}
