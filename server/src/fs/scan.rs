use std::fmt::{Debug, Display};
use std::fs::{canonicalize, metadata};
use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::sync::Arc;

use blockhash::blockhash256;
use chrono::Local;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader};
use tokio::{sync::RwLock, task::JoinSet};
use tracing::{debug, info, instrument, warn, Level};
use walkdir::WalkDir;

use crate::db::msg::DbMsg;
use crate::fs::{media_original_path, media_thumbnail_path};
use crate::service::ESMSender;
use api::{
    library::{LibraryScanJob, LibraryUuid},
    media::{Media, MediaMetadata, MediaUuid},
};
use common::config::ESConfig;

#[derive(Clone, Debug)]
pub struct ScanContext {
    pub config: Arc<ESConfig>,
    pub library_uuid: LibraryUuid,
    pub library_path: PathBuf,
    pub db_svc_sender: ESMSender,
    pub job_status: Arc<RwLock<LibraryScanJob>>,
}

impl ScanContext {
    async fn error(&self, path: impl Debug, msg: impl Display, error: impl Debug) -> () {
        // a scan error will not stop the scanner thread, nevermind the fs service or main,
        // so we record these errors as warnings
        warn!({path = ?path, error = ?error}, "{msg}");

        let mut status = self.job_status.write().await;

        status.error_count += 1;
    }

    async fn count_inc(&self) -> () {
        let mut status = self.job_status.write().await;

        status.file_count += 1;
    }
}

struct MediaData {
    hash: String,
    date: String,
    metadata: MediaMetadata,
}

// concurrent file scanner
#[instrument(level=Level::DEBUG, skip(context))]
pub async fn run_scan(context: Arc<ScanContext>) -> () {
    let context = context.clone();

    info!({ library_uuid = context.library_uuid, library_path = ?context.library_path }, "starting scan");

    let mut tasks = JoinSet::new();

    let max_threads = context.config.clone().fs_scanner_threads;

    for entry in WalkDir::new(context.library_path.clone())
        .same_file_system(true)
        .contents_first(true)
        .into_iter()
    {
        // things to check eventually:
        //  * too many errors
        //  * cancel signal
        if tasks.len() > max_threads {
            tasks.join_next().await;
        }

        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                context
                    .error("unknown path", "failed to parse DirEntry", err)
                    .await;
                continue;
            }
        };

        let path = match canonicalize(entry.path()) {
            Ok(path) => path,
            Err(err) => {
                context
                    .error(entry.path(), "failed to canonicalize path", err)
                    .await;
                continue;
            }
        };

        let meta = match metadata(&path) {
            Ok(meta) => meta,
            Err(err) => {
                context
                    .error(entry.path(), "failed to parse metadata", err)
                    .await;
                continue;
            }
        };

        if meta.is_dir() {
            // look for starlark files to run, possibly with a specific name
            //
            // note thet library.uid will need to be plumbed through to here
            // so that we can actually use the relevant DbMsg calls
            continue;
        } else if meta.is_file() {
            tasks.spawn(register_media(context.clone(), path));
        } else {
            // technically, this should be unreachable, but we want to cover the eventuality
            // that the behavior of is_dir()/is_file() change
            context
                .error(
                    entry.path(),
                    "direntry is neither file nor directory",
                    "custom",
                )
                .await;
            continue;
        };
    }

    // wait for all the tasks to complete
    tasks.join_all().await;
}

// media registration
//
// this function is spawned into a tokio task set, hence the empty return type
// and use of the scan context to actually communicate with the caller
#[instrument(level=Level::DEBUG, skip(context, path))]
async fn register_media(context: Arc<ScanContext>, path: PathBuf) -> () {
    debug!({path = ?path}, "started processing media");

    // first, check if the media already exists in the database
    let pathstr = match path.to_str() {
        Some(pathstr) => pathstr.to_owned(),
        None => {
            context
                .error(path, "failed to convert path to str", "custom")
                .await;
            return;
        }
    };

    let (tx, rx) = tokio::sync::oneshot::channel();

    match context
        .db_svc_sender
        .send(
            DbMsg::GetMediaUuidByPath {
                resp: tx,
                path: pathstr.clone(),
            }
            .into(),
        )
        .await
    {
        Ok(_) => context.count_inc().await,
        Err(err) => {
            context
                .error(pathstr, "failed to send GetMediaUuidByPath message", err)
                .await;
            return;
        }
    }

    match rx.await {
        Ok(resp) => match resp {
            Ok(exists) => match exists {
                Some(_) => return,
                None => {}
            },
            Err(err) => {
                context
                    .error(
                        pathstr,
                        "failure when searching for media_uuid in database",
                        err,
                    )
                    .await;
                return;
            }
        },
        Err(err) => {
            context
                .error(
                    pathstr,
                    "failed to get response for GetMediaUuidByPath message",
                    err,
                )
                .await;
            return;
        }
    }

    // if the file is new, we need to call the correct metadata collector for its file extension
    let ext = match path.extension() {
        Some(extstr) => match extstr.to_str() {
            Some(val) => val,
            None => {
                context
                    .error(path, "failed to convert file extension", "custom")
                    .await;
                return;
            }
        },
        None => {
            context
                .error(path, "failed to find file extension", "custom")
                .await;
            return;
        }
    };

    let mediadata: Result<MediaData, anyhow::Error> = match ext {
        "jpg" | "png" | "tiff" => process_image(context.config.clone(), path.clone()).await,
        _ => {
            context
                .error(
                    path.clone(),
                    format!("failed to match {ext} to known file types"),
                    "custom",
                )
                .await;
            return;
        }
    };

    // async closures have stabilized, but i don't remember where we wanted to use it
    let mediadata: MediaData = match mediadata {
        Ok(val) => val,
        Err(err) => {
            context
                .error(path, "failed to process media metadata", err)
                .await;
            return;
        }
    };

    // once we have the metadata, we assemble the Media struct and send it to the database
    let media = Media {
        library_uuid: context.library_uuid,
        path: pathstr.clone(),
        hash: mediadata.hash,
        mtime: Local::now().timestamp(),
        hidden: false,
        date: mediadata.date,
        note: "".to_owned(),
        metadata: mediadata.metadata.clone(),
    };

    let (tx, rx) = tokio::sync::oneshot::channel();

    match context
        .db_svc_sender
        .send(
            DbMsg::AddMedia {
                resp: tx,
                media: media,
            }
            .into(),
        )
        .await
    {
        Ok(_) => {}
        Err(err) => {
            context
                .error(path, "failed to send AddMedia message", err)
                .await;
            return;
        }
    };

    // finally, if the media registers properly, we can use the uuid to make it accessible
    let media_uuid: MediaUuid = match rx.await {
        Ok(resp) => match resp {
            Ok(val) => val,
            Err(err) => {
                context
                    .error(path, "failure when adding media to database", err)
                    .await;
                return;
            }
        },
        Err(err) => {
            context
                .error(path, "failed to get response for AddMedia message", err)
                .await;
            return;
        }
    };

    // TODO -- change to relative by adjusting original path
    match symlink(
        path.clone(),
        media_original_path(context.config.clone(), media_uuid.to_string()),
    ) {
        Ok(_) => {}
        Err(err) => {
            context.error(path, "failed to add symlink", err).await;
            return;
        }
    }

    match create_thumbnail(
        context.config.clone(),
        path.clone(),
        media_uuid,
        mediadata.metadata,
    ) {
        Ok(_) => {}
        Err(err) => {
            context.error(path, "failed to create thumbnail", err).await;
            return;
        }
    }

    context.count_inc().await;
    debug!({path = ?path}, "finished processing media");
}

async fn process_image(_config: Arc<ESConfig>, path: PathBuf) -> anyhow::Result<MediaData> {
    debug!({path = ?path}, "starting processing image");

    // exif processing
    //
    // we attempt to read the exif metadata for the image to extract the date
    // following the exif docs, open the file synchronously and read from the container
    let file = std::fs::File::open(&path)?;

    let mut bufreader = std::io::BufReader::new(file);

    let exifreader = exif::Reader::new();

    let exif = exifreader.read_from_container(&mut bufreader)?;

    // process the exif fields
    let datetime_original = match exif.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY) {
        Some(dto) => format!("{}", dto.display_value()),
        None => String::from(""),
    };

    // perceptual hashing
    let image = image::open(&path)?;

    let hash = blockhash256(&image);

    debug!({path = ?path}, "finshed processing image");

    Ok(MediaData {
        hash: hash.to_string(),
        date: datetime_original,
        metadata: MediaMetadata::Image,
    })
}

fn create_thumbnail(
    config: Arc<ESConfig>,
    path: PathBuf,
    media_uuid: MediaUuid,
    media_metadata: MediaMetadata,
) -> anyhow::Result<()> {
    debug!({path = ?path}, "started creating thumbnail");

    let mut decoder = match media_metadata {
        MediaMetadata::Image => ImageReader::open(path.clone())?.into_decoder()?,
        _ => return Err(anyhow::Error::msg("not implemented")),
    };

    // this both solves the crate version collision and corrects the orientation, too
    let orientation = decoder.orientation()?;

    debug!({path = ?path, orientation = ?orientation}, "orientation for image");

    let image = DynamicImage::from_decoder(decoder)?;

    // create the thumbnail with bounds, not exact sizing
    let mut thumbnail = image.thumbnail(400, 400);

    thumbnail.apply_orientation(orientation);

    thumbnail.save_with_format(
        media_thumbnail_path(config, media_uuid.to_string()),
        ImageFormat::Png,
    )?;

    debug!({path = ?path}, "finished creating thumbnail");

    Ok(())
}
