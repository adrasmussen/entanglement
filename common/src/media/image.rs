use std::path::PathBuf;

use blockhash::blockhash256;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader};
use tracing::debug;

use crate::media::MediaData;
use api::media::MediaMetadata;

pub async fn process_image(path: &PathBuf) -> anyhow::Result<MediaData> {
    debug!("starting processing image");

    // exif processing
    //
    // we attempt to read the exif metadata for the image to extract the date
    // following the exif docs, open the file synchronously and read from the container
    let file = std::fs::File::open(path)?;

    let mut bufreader = std::io::BufReader::new(file);

    let exifreader = exif::Reader::new();

    let exif = exifreader.read_from_container(&mut bufreader)?;

    // process the exif fields
    let datetime_original = match exif.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY) {
        Some(dto) => format!("{}", dto.display_value()),
        None => String::from(""),
    };

    // perceptual hashing
    let image = image::open(path)?;

    let hash = blockhash256(&image);

    debug!("finshed processing image");

    Ok(MediaData {
        hash: hash.to_string(),
        date: datetime_original,
        metadata: MediaMetadata::Image,
    })
}

pub fn create_image_thumbnail(
    original_path: PathBuf,
    thumbnail_path: PathBuf,
) -> anyhow::Result<()> {
    debug!("started creating thumbnail");

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
}
