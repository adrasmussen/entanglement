use std::collections::HashMap;
use std::fs::read_dir;
use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::{self, Context};

use async_recursion::async_recursion;
use async_trait::async_trait;

use chrono::NaiveDateTime;

use crate::db::msg::DbMsg;
use crate::fs::*;
use crate::service::*;
use api::{image::*, MediaUuid};

use super::msg::FsMsg;

pub struct FileScanner {
    db_svc_sender: ESMSender,
    media_srcdir: PathBuf,
    media_linkdir: PathBuf,
}

#[async_trait]
impl ESFileService for FileScanner {
    async fn scan_library(&self, user: String) -> anyhow::Result<()> {
        // this needs to be modified to call scan_manager() instead, and
        // we probably want some internal state to keep track of how
        // many scans are running
        //
        // it should also take a Library object as the argument

        todo!()
    }

    async fn rescan_file(&self, file: PathBuf) -> anyhow::Result<()> {
        // need to check that the dbmsg is idempotent

        let user = "todo -- either pass in the owner or get it from db".to_string();

        let scan_info = ScanInfo {
            db_svc_sender: self.db_svc_sender.clone(),
            user: user.clone(),
            library: user.clone(),
            media_linkdir: self.media_linkdir.clone(),
        };

        let _ = register_media(&scan_info, file);

        Ok(())
    }

    async fn fix_symlinks(&self) -> anyhow::Result<()> {
        todo!()
    }
}

#[async_trait]
impl ESInner for FileScanner {
    fn new(
        config: Arc<ESConfig>,
        senders: HashMap<ServiceType, ESMSender>,
    ) -> anyhow::Result<Self> {
        Ok(FileScanner {
            db_svc_sender: senders.get(&ServiceType::Db).unwrap().clone(),
            media_srcdir: config.media_srcdir.clone(),
            media_linkdir: config.media_linkdir.clone(),
        })
    }

    async fn message_handler(&self, esm: ESM) -> anyhow::Result<()> {
        match esm {
            ESM::Fs(message) => match message {
                FsMsg::Status { resp } => self.respond(resp, async { todo!() }).await,
                FsMsg::ScanLibrary { resp, library } => {
                    self.respond(resp, self.scan_library(library)).await
                }
                FsMsg::RescanFile { resp, file } => {
                    self.respond(resp, self.rescan_file(file)).await
                }
                FsMsg::FixSymlinks { resp } => self.respond(resp, self.fix_symlinks()).await,
            },
            _ => Err(anyhow::Error::msg("not implemented")),
        }
    }
}

struct ScanInfo {
    db_svc_sender: ESMSender,
    user: String,
    library: String,
    media_linkdir: PathBuf,
}

struct ScanResult {
    count: i64,
    failures: HashMap<String, String>,
}

async fn scan_manager() -> anyhow::Result<ScanResult> {
    // could get total number of files here + LastScanDate with atime in library and use count to estimate time remaining
    todo!()
}

// this function does not currently have a good way of communicating errors back to the caller,
// since we don't want the whole scan to abort if we run into an issue
//
// the easiest way would be to just have an outer function and send a channel in via scan_info
#[async_recursion]
async fn scan_directory(scan_info: &ScanInfo, path: PathBuf) -> anyhow::Result<()> {
    for entry in read_dir(path)? {
        let entry = entry?;
        let meta = entry.metadata()?;

        if meta.is_dir() {
            match scan_directory(scan_info, entry.path()).await {
                Ok(()) => {}
                Err(err) => println!(
                    "failed to enter directory {}: {}",
                    entry
                        .file_name()
                        .to_str()
                        .unwrap_or_else(|| "invalid directory name"),
                    err
                ),
            }
        }

        if meta.is_file() {
            match register_media(scan_info, entry.path()).await {
                Ok(()) => {}
                Err(err) => println!(
                    "failed to register {}: {}",
                    entry
                        .file_name()
                        .to_str()
                        .unwrap_or_else(|| "invalid filename"),
                    err
                ),
            }
        }
    }

    Ok(())
}

// we want to have a uniform way of handling the various sorts of files that we might
// encounter while looking for media, and this
async fn register_media(scan_info: &ScanInfo, path: PathBuf) -> anyhow::Result<()> {
    let ext = path
        .extension()
        .ok_or_else(|| anyhow::Error::msg(format!("missing file extension for {path:?}")))?
        .to_str()
        .ok_or_else(|| {
            anyhow::Error::msg(format!("failed to convert file extension for {path:?}"))
        })?;

    let uuid: MediaUuid = match ext {
        ".jpg" | ".png" | ".tiff" => register_image(scan_info, &path).await?,
        _ => {
            return Err(anyhow::Error::msg(format!(
                "unknown file extension for {path:?}"
            )))
        }
    };

    // this should probably be another helper function so that the http server can easily
    // map uuid -> path without relying on magic numbers
    let link = scan_info.media_linkdir.join(uuid.to_string());

    symlink(path, link).context(format!("Failed to create symlink for {uuid}"))
}

// it seems that the exif package is synchronous only because the read_from_container() requires
// BufRead instead of AsyncBufRead
//
// we may have to fix this
//
// TODO -- create the thumbnail here -> move symlink too?
async fn register_image(scan_info: &ScanInfo, path: &PathBuf) -> anyhow::Result<MediaUuid> {
    use exif::{In, Reader, Tag};

    // attempt to convert the path (this may fail, and if it does, don't bother actually opening and reading the file)
    let pathstr = path
        .to_str()
        .ok_or_else(|| anyhow::Error::msg("Failed to convert path to str"))?
        .to_string();

    // following the exif docs, open the file synchronously and read from the container
    let file = std::fs::File::open(&path).context(format!("Failed to open file {path:?}"))?;

    let mut bufreader = std::io::BufReader::new(file);

    let exifreader = Reader::new();

    let exif = exifreader.read_from_container(&mut bufreader)?;

    // process the exif fields
    let datetime_original = match exif.get_field(Tag::DateTimeOriginal, In::PRIMARY) {
        Some(dto) => exif_datetime_to_utc(&format!("{}", dto.display_value()))?,
        None => 0,
    };

    let x_pixel = match exif.get_field(Tag::PixelXDimension, In::PRIMARY) {
        Some(xpi) => xpi
            .value
            .get_uint(0)
            .ok_or_else(|| anyhow::Error::msg("Failed to convert x_pixels"))?,
        None => 0,
    };

    let y_pixel = match exif.get_field(Tag::PixelYDimension, In::PRIMARY) {
        Some(ypi) => ypi
            .value
            .get_uint(0)
            .ok_or_else(|| anyhow::Error::msg("Failed to convert y_pixels"))?,
        None => 0,
    };

    let orientation = match exif.get_field(Tag::Orientation, In::PRIMARY) {
        Some(ort) => ort.value.get_uint(0).unwrap_or_else(|| 0),
        None => 0,
    };

    let datetime = match exif.get_field(Tag::DateTimeDigitized, In::PRIMARY) {
        Some(dto) => format!("{}", dto.display_value()),
        None => "".to_string(),
    };

    // assemble the database message
    let image = Image {
        data: ImageData {
            owner: scan_info.user.clone(),
            path: pathstr,
            datetime_original: datetime_original,
            x_pixel: x_pixel,
            y_pixel: y_pixel,
        },
        metadata: ImageMetadata {
            orientation: Some(orientation),
            date: Some(datetime),
            note: Some("".to_string()),
        },
    };

    let (tx, rx) = tokio::sync::oneshot::channel();

    scan_info
        .db_svc_sender
        .clone()
        .send(
            DbMsg::AddImage {
                resp: tx,
                image: image,
            }
            .into(),
        )
        .await
        .context(format!("Failed to send AddImage for {path:?}"))?;

    let uuid = rx
        .await
        .context(format!("Failed to receive AddImage response for {path:?}"))??;

    Ok(uuid)
}

fn exif_datetime_to_utc(date_str: &str) -> anyhow::Result<i64> {
    Ok(NaiveDateTime::parse_from_str(date_str, "%Y-%m-%d %H:%M:%S")
        .map_err(|_| anyhow::Error::msg("failed to parse datetime"))?
        .and_utc()
        .timestamp())
}
