use std::fs::{canonicalize, metadata, read_dir};
use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::sync::Arc;

use async_recursion::async_recursion;

use crate::db::msg::DbMsg;
use crate::service::ESMSender;
use api::{
    library::LibraryUuid,
    media::{Media, MediaMetadata, MediaUuid},
};

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

#[async_recursion]
pub async fn scan_directory(
    scan_context: Arc<ScanContext>,
    dir_path: PathBuf,
) -> Result<(), ScanError> {
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
            joinset.spawn(scan_directory(scan_context.clone(), path));
        } else if meta.is_file() {
            joinset.spawn(register_media(scan_context.clone(), path));
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
                });
            }
            Ok(res) => match scan_context.scan_sender.send(res).await {
                Ok(()) => continue,
                // this error is somewhat harder to deal with
                //
                // it should probably be replaced with an appropriate error log
                Err(_) => {
                    return Err(ScanError {
                        path: dir_path.clone(),
                        info: String::from("Failed to send result to scan listener"),
                    })
                }
            },
        }
    }

    Ok(())
}

// errors generated here are handled by scan_directory
async fn register_media(scan_context: Arc<ScanContext>, path: PathBuf) -> Result<(), ScanError> {
    // first, check if the media already exists in the database
    let pathstr = path
        .to_str()
        .ok_or_else(|| ScanError {
            path: path.clone(),
            info: String::from("Failed to convert path to str"),
        })?
        .to_owned();

    let (tx, rx) = tokio::sync::oneshot::channel();

    scan_context
        .db_svc_sender
        .send(
            DbMsg::GetMediaUuidByPath {
                resp: tx,
                path: pathstr.clone(),
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

    // if the file is new, we need to call the correct metadata collector for its file extension
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

    let media_metadata: MediaMetadata = match ext {
        ".jpg" | ".png" | ".tiff" => get_image_metadata(path.clone()).await?,
        _ => {
            return Err(ScanError {
                path: path.clone(),
                info: String::from("Failed to match file extension to known types"),
            })
        }
    };

    // once we have the metadata, we assemble the Media struct and send it to the database
    let media = Media {
        library_uuid: scan_context.library_uuid,
        path: pathstr,
        hidden: false,
        metadata: media_metadata,
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
            info: String::from("Failed to send AddMedia message from register_media"),
        })?;

    // finally, if the media registers properly, we can use the uuid to make it accessible
    let media_uuid: MediaUuid = rx
        .await
        .map_err(|_| ScanError {
            path: path.clone(),
            info: String::from("Failed to receive AddMedia response at register_media"),
        })?
        .map_err(|err| ScanError {
            path: path.clone(),
            info: format!("Failure when adding media to database: {}", err.to_string()),
        })?;

    // this should probably be another helper function so that the http server can easily
    // map uuid -> path without relying on magic numbers
    let link = scan_context.media_linkdir.join("full").join(media_uuid.to_string());

    // TODO -- change to relative by adjusting original path
    symlink(path.clone(), link).map_err(|err| ScanError {
        path: path.clone(),
        info: format!("Failed to create symlink: {}", err.to_string()),
    })
}

async fn get_image_metadata(path: PathBuf) -> Result<MediaMetadata, ScanError> {
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

    Ok(MediaMetadata {
        date: datetime_original,
        note: String::from(""),
    })
}
