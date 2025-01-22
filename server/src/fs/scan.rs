use std::fmt::Display;
use std::fs::{canonicalize, metadata};
use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::Local;
use tokio::{sync::RwLock, task::JoinSet};
use walkdir::WalkDir;

use crate::db::msg::DbMsg;
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
    pub media_linkdir: PathBuf,
    pub job_status: Arc<RwLock<LibraryScanJob>>,
}

impl ScanContext {
    async fn error(&self, msg: impl Display) -> () {
        let mut status = self.job_status.write().await;

        println!("scan error: {msg}");

        status.error_count += 1;
    }
}

pub async fn run_scan(context: Arc<ScanContext>) -> () {
    let context = context.clone();

    let mut tasks = JoinSet::new();

    for entry in WalkDir::new(context.library_path.clone()).into_iter() {
        // things to check eventually:
        //  * too many errors
        //  * cancel signal
        if !tasks.len() < 8 {
            tasks.join_next().await;
        }

        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                context
                    .error(format!("failed to parse DirEntry {err}"))
                    .await;
                continue;
            }
        };

        let path = match canonicalize(entry.path()) {
            Ok(path) => path,
            Err(err) => {
                context
                    .error(format!(
                        "failed to canonicalize for {:?}: {err}",
                        entry.path()
                    ))
                    .await;
                continue;
            }
        };

        let meta = match metadata(&path) {
            Ok(meta) => meta,
            Err(err) => {
                context
                    .error(format!(
                        "failed to parse metadata for {:?}: {err}",
                        entry.path()
                    ))
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
                .error(format!("{:?} is neither file nor directory", entry.path()))
                .await;
            continue;
        };
    }
}

async fn register_media(context: Arc<ScanContext>, path: PathBuf) -> () {
    // first, check if the media already exists in the database
    let pathstr = match path.to_str() {
        Some(pathstr) => pathstr.to_owned(),
        None => {
            context
                .error(format!("failed to convert {path:?} to str"))
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
        Ok(_) => {}
        Err(err) => {
            context.error(format!("{err}")).await;
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
                    .error(format!(
                        "failure when searching for media_uuia in database: {err}"
                    ))
                    .await;
                return;
            }
        },
        Err(err) => {
            context.error(format!("{err}")).await;
            return;
        }
    }

    // if the file is new, we need to call the correct metadata collector for its file extension
    let ext = match path.extension() {
        Some(extstr) => match extstr.to_str() {
            Some(val) => val,
            None => {
                context
                    .error(format!("failed to convert file extension for {path:?}"))
                    .await;
                return;
            }
        },
        None => {
            context
                .error(format!("failed to find file extension for {path:?}"))
                .await;
            return;
        }
    };

    let media_metadata: Result<(MediaMetadata, Option<String>), anyhow::Error> = match ext {
        ".jpg" | ".png" | ".tiff" => process_image(context.config.clone(), path.clone()).await,
        _ => {
            context
                .error(format!("failed to match {ext} to known file types"))
                .await;
            return;
        }
    };

    let media_metadata: (MediaMetadata, Option<String>) = match media_metadata {
        Ok(val) => val,
        Err(err) => {
            context
                .error(format!(
                    "failed to process media metadata for {path:?}: {err}"
                ))
                .await;
            return;
        }
    };

    // calculate the hash
    let hash = "OH NO".to_owned();

    // once we have the metadata, we assemble the Media struct and send it to the database
    let media = Media {
        library_uuid: context.library_uuid,
        path: pathstr,
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
            context.error(format!("{err}")).await;
            return;
        }
    };

    // finally, if the media registers properly, we can use the uuid to make it accessible
    let media_uuid: MediaUuid = match rx.await {
        Ok(resp) => match resp {
            Ok(val) => val,
            Err(err) => {
                context
                    .error(format!("failure when adding media to database: {err}"))
                    .await;
                return;
            }
        },
        Err(err) => {
            context.error(format!("{err}")).await;
            return;
        }
    };

    // this should probably be another helper function so that the http server can easily
    // map uuid -> path without relying on magic numbers
    let link = context
        .media_linkdir
        .join("full")
        .join(media_uuid.to_string());

    // TODO -- change to relative by adjusting original path
    match symlink(path.clone(), link) {
        Ok(_) => {}
        Err(err) => {
            context
                .error(format!("failed to add symlink for {path:?}: {err}"))
                .await;
            return;
        }
    }
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
