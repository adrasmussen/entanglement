use std::fmt::{Debug, Display};
use std::fs::{canonicalize, metadata};
use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::Local;
use tokio::{sync::RwLock, task::JoinSet};
use tracing::{debug, error, info, instrument};
use walkdir::WalkDir;

use crate::db::msg::DbMsg;
use crate::service::ESMSender;
use api::{
    library::{LibraryScanJob, LibraryUuid},
    media::{Media, MediaMetadata, MediaUuid},
    ORIGINAL_PATH,
};
use common::config::ESConfig;

#[derive(Clone, Debug)]
pub struct ScanContext {
    pub config: Arc<ESConfig>,
    pub library_uuid: LibraryUuid,
    pub library_path: PathBuf,
    pub db_svc_sender: ESMSender,
    pub media_linkdir: PathBuf,
    pub job_status: Arc<RwLock<LibraryScanJob>>,
}

impl ScanContext {
    async fn error(&self, path: impl Debug, msg: impl Display, error: impl Debug) -> () {
        error!({path = ?path, error = ?error}, "{msg}");

        let mut status = self.job_status.write().await;

        status.error_count += 1;
    }

    async fn count_inc(&self) -> () {
        let mut status = self.job_status.write().await;

        status.file_count += 1;
    }
}

#[instrument(skip(context))]
pub async fn run_scan(context: Arc<ScanContext>) -> () {
    let context = context.clone();

    info!({ library_uuid = context.library_uuid, library_path = ?context.library_path }, "starting scan");

    let mut tasks = JoinSet::new();

    for entry in WalkDir::new(context.library_path.clone()).into_iter() {
        // things to check eventually:
        //  * too many errors
        //  * cancel signal
        if tasks.len() > 8 {
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

#[instrument(skip(context, path))]
async fn register_media(context: Arc<ScanContext>, path: PathBuf) -> () {
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

    let media_metadata: Result<(MediaMetadata, Option<String>), anyhow::Error> = match ext {
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

    let media_metadata: (MediaMetadata, Option<String>) = match media_metadata {
        Ok(val) => val,
        Err(err) => {
            context
                .error(path, "failed to process media metadata", err)
                .await;
            return;
        }
    };

    // calculate the hash
    let hash = "OH NO".to_owned();

    // once we have the metadata, we assemble the Media struct and send it to the database
    let media = Media {
        library_uuid: context.library_uuid,
        path: pathstr.clone(),
        hash: hash,
        mtime: Local::now().timestamp(),
        hidden: false,
        attention: false,
        date: media_metadata
            .1
            .unwrap_or_else(|| "get from parser".to_owned()),
        note: "".to_owned(),
        metadata: media_metadata.0,
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

    // this should probably be another helper function so that the http server can easily
    // map uuid -> path without relying on magic numbers
    let link = context
        .media_linkdir
        .join(ORIGINAL_PATH)
        .join(media_uuid.to_string());

    // TODO -- change to relative by adjusting original path
    match symlink(path.clone(), link) {
        Ok(_) => {}
        Err(err) => {
            context.error(path, "failed to add symlink", err).await;
            return;
        }
    }

    context.count_inc().await;
}

async fn process_image(
    config: Arc<ESConfig>,
    path: PathBuf,
) -> anyhow::Result<(MediaMetadata, Option<String>)> {
    use exif::{In, Reader, Tag};

    // following the exif docs, open the file synchronously and read from the container
    let file = std::fs::File::open(&path)?;

    let mut bufreader = std::io::BufReader::new(file);

    let exifreader = Reader::new();

    let exif = exifreader.read_from_container(&mut bufreader)?;

    // process the exif fields
    let datetime_original = match exif.get_field(Tag::DateTimeOriginal, In::PRIMARY) {
        Some(dto) => format!("{}", dto.display_value()),
        None => String::from(""),
    };

    Ok((MediaMetadata::Image, Some(datetime_original)))
}
