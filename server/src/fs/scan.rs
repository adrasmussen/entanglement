use std::fs::{canonicalize, metadata, read_dir};
use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::sync::Arc;

use async_recursion::async_recursion;

use crate::db::msg::DbMsg;
use crate::service::ESMSender;
use api::{library::LibraryUuid, Media, MediaMetadata, MediaUuid};

pub struct ScanContext {
    pub scan_sender: tokio::sync::mpsc::Sender<Result<(), ScanError>>,
    pub db_svc_sender: ESMSender,
    pub library_uuid: LibraryUuid,
    pub media_linkdir: PathBuf,
}

#[derive(Clone, Debug)]
pub struct ScanError {
    pub path: PathBuf,
    pub info: String,
}

#[derive(Clone, Debug)]
pub struct ScanReport {
    pub count: i64,
    pub errors: Vec<ScanError>,
}

#[async_recursion]
pub async fn scan_directory(scan_context: Arc<ScanContext>, dir_path: PathBuf) -> Result<(), ScanError> {
    let mut joinset = tokio::task::JoinSet::new();

    let contents = match read_dir(dir_path.clone()) {
        Ok(val) => val,
        Err(err) => {
            return Err(ScanError {
                path: dir_path.clone(),
                info: format!("Failed to read directory contents: {}", err.to_string()),
            })
        }
    };

    for entry in contents {
        let entry = entry.map_err(|err| ScanError {
            path: dir_path.clone(),
            info: format!("Failed to entry in directory: {}", err.to_string()),
        })?;

        let path = canonicalize(entry.path()).map_err(|err| ScanError {
            path: entry.path().clone(),
            info: format!("Failed to canonicalize path: {}", err.to_string()),
        })?;

        let meta = metadata(&path).map_err(|err| ScanError {
            path: path.clone(),
            info: format!("Failed to get metadata: {}", err.to_string()),
        })?;

        if meta.is_dir() {
            joinset.spawn(scan_directory(scan_context
        .clone(), path));
        } else if meta.is_file() {
            joinset.spawn(register_media(scan_context
        .clone(), path));
        } else {
            // technically, this should be unreachable, but we want to cover the eventuality
            // that the behavior of is_dir()/is_file() change
            return Err(ScanError {
                path: path,
                info: String::from("Failed to determine if path or dir"),
            });
        };
    }

    while let Some(join_res) = joinset.join_next().await {
        match join_res {
            Err(err) => {
                // this should also be unreachable, at least until we include logic to cancel
                // running scans (or accidentally introduce a scan function that can panic)
                return Err(ScanError {
                    path: dir_path.clone(),
                    info: format!("Failed to join process handle: {}", err.to_string()),
                })
            }
            Ok(res) => match scan_context.scan_sender.send(res).await {
                Ok(()) => continue,
                // this error is somewhat harder to deal with
                //
                // it should probably be replaced with an appropriate error log
                Err(_) => return Err(ScanError {
                    path: dir_path.clone(),
                    info: String::from("Failed to send result to scan listener"),
                })

            }
        }
    }

    Ok(())
}

// errors generated here are handled by scan_directory
async fn register_media(scan_context: Arc<ScanContext>, path: PathBuf) -> Result<(), ScanError> {
    let pathstr = path.to_str().ok_or_else(|| ScanError {
        path: path.clone(),
        info: String::from("Failed to convert path to str"),
    })?;

    let (tx, rx) = tokio::sync::oneshot::channel();

    scan_context
        .db_svc_sender
        .send(
            DbMsg::GetMediaUuidByPath {
                resp: tx,
                path: pathstr.into(),
            }
            .into(),
        )
        .await
        .map_err(|_| ScanError {
            path: path.clone(),
            info: String::from("Failed to send GetMediaByPath message from register_media"),
        })?;

    match rx
        .await
        .map_err(|_| ScanError {
            path: path.clone(),
            info: String::from("Failed to receive GetMediaByPath response at register_media"),
        })?
        .map_err(|err| ScanError {
            path: path.clone(),
            info: format!(
                "Failure when searching for media uuid in database: {}",
                err.to_string()
            ),
        })? {
        Some(_) => return Ok(()),
        None => {}
    }

    let ext = path
        .extension()
        .ok_or_else(|| ScanError {
            path: path.clone(),
            info: String::from("Failed to find file extension"),
        })?
        .to_str()
        .ok_or_else(|| ScanError {
            path: path.clone(),
            info: String::from("Failed to convert file extension"),
        })?;

    let uuid: MediaUuid = match ext {
        ".jpg" | ".png" | ".tiff" => register_image(scan_context
    .clone(), path.clone()).await?,
        _ => {
            return Err(ScanError {
                path: path.clone(),
                info: String::from("Failed to match file extension to known types"),
            })
        }
    };

    // this should probably be another helper function so that the http server can easily
    // map uuid -> path without relying on magic numbers
    let link = scan_context.media_linkdir.join(uuid.to_string());

    match symlink(path.clone(), link) {
        Ok(()) => Ok(()),
        Err(err) => Err(ScanError {
            path: path.clone(),
            info: format!("Failed to create symlink: {}", err.to_string()),
        }),
    }
}

async fn register_image(scan_context: Arc<ScanContext>, path: PathBuf) -> Result<i64, ScanError> {
    use exif::{In, Reader, Tag};

    // following the exif docs, open the file synchronously and read from the container
    let file = std::fs::File::open(&path).map_err(|err| ScanError {
        path: path.clone(),
        info: format!("Failed to open file: {}", err.to_string()),
    })?;

    let mut bufreader = std::io::BufReader::new(file);

    let exifreader = Reader::new();

    let exif = exifreader
        .read_from_container(&mut bufreader)
        .map_err(|err| ScanError {
            path: path.clone(),
            info: format!("Failed to read from exif container: {}", err.to_string()),
        })?;

    // process the exif fields
    let datetime_original = match exif.get_field(Tag::DateTimeOriginal, In::PRIMARY) {
        Some(dto) => format!("{}", dto.display_value()),
        None => String::from(""),
    };

    let media = Media {
        library_uuid: scan_context
.library_uuid,
        path: path.clone(),
        hidden: false,
        metadata: MediaMetadata {
            date: datetime_original,
            note: String::from(""),
        },
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    scan_context
        .db_svc_sender
        .send(
            DbMsg::AddMedia {
                resp: tx,
                media: media,
            }
            .into(),
        )
        .await
        .map_err(|_| ScanError {
            path: path.clone(),
            info: String::from("Failed to send AddMedia message from register_image"),
        })?;

    let uuid = rx
        .await
        .map_err(|_| ScanError {
            path: path.clone(),
            info: String::from("Failed to receive AddMedia response at register_image"),
        })?
        .map_err(|err| ScanError {
            path: path,
            info: format!("Failure when adding media to database: {}", err.to_string()),
        })?;

    Ok(uuid)
}
