use std::collections::HashMap;
use std::sync::Arc;

use async_cell::sync::AsyncCell;

use async_trait::async_trait;

use futures::{future::BoxFuture, FutureExt};

use mysql_async::{from_row_opt, prelude::*, BinaryProtocol, Opts, Pool, ResultSetStream};

use tokio::sync::Mutex;

use crate::db::{msg::DbMsg, ESDbConn, ESDbService};
use crate::service::{
    ESConfig, ESInner, ESMReceiver, ESMResp, ESMSender, EntanglementService, ESM,
};
use api::*;

pub struct MySQLState {
    pool: Pool,
}

// database RPC handler functions
//
// these functions take a somewhat strange form to ensure that we can correctly capture all errors,
// either to pass them back to the client or to log them in the server logs
//
// it's entirely possible that some of these *should* be unwraps, since being unable to respond to
// inter-service messages is a good reason to halt the server process.  however, this method gives
// us more flexibilty, since a failure can instead cause the server to gracefully stop other tasks
//
// thus, we have the inner async {} -> Result and resp.send(inner.await)
//
// the other somewhat unfortunate pattern is having to manipulate the query result iterator so we
// can use from_row_opt() instead of prepackaged Query::first(conn), fetch(conn), and other tools
//
// every query needs the run(conn).await? portion, which actually executes the query and returns
// the result iterator, which is more complicated because there are result "sets"
//
// several of the internal mechanisms call .next().await?, which moves through result sets and
// fails if they have been otherwise consumed by something else from that connection
//
// if we do so manually (like wanting just the first result), we have to unpack the Option<> first
// and then the Result<_, FromRowError> on the inside

#[async_trait]
impl ESDbService for MySQLState {
    async fn add_image(&self, resp: ESMResp<ImageUuid>, image: Image) -> anyhow::Result<()> {
        let inner = async {
            let conn = self.pool.get_conn().await?;

            let visibility: String = image.metadata.visibility.ok_or_else(|| anyhow::Error::msg("missing visibility"))?.into();
            let orientation: i32 = image.metadata.orientation.ok_or_else(|| anyhow::Error::msg("missing orientation"))?;
            let date: u64 = image.metadata.date.ok_or_else(|| anyhow::Error::msg("missing date"))?;
            let note: String = image.metadata.note.ok_or_else(|| anyhow::Error::msg("missing note"))?;

            let result: u64 = r"
                INSERT INTO images (uuid, owner, path, size, mtime, x_pixel, y_pixel, visibilty, orientation, date, note)
                OUTPUT INSERTED.uuid
                VALUES (UUID_SHORT(), :owner, :path, :size, :mtime, :x_pixel, :y_pixel, :visibilty, :orientation, :date, :note)"
                .with(params! {
                    "owner" => image.file.owner,
                    "path" => image.file.path,
                    "size" => image.file.size,
                    "mtime" => image.file.mtime,
                    "x_pixel" => image.file.x_pixel,
                    "y_pixel" => image.file.y_pixel,
                    "visibility" => visibility,
                    "orientation" => orientation,
                    "date" => date,
                    "note" => note,
                })
                .run(conn).await?
                .next().await?
                .map(|row| from_row_opt(row))
                .ok_or_else(|| anyhow::Error::msg(format!("failed to return inserted uuid")))??;

            Ok(result)
        };

        resp.send(inner.await).map_err(|_| anyhow::Error::msg("failed to respond to add_image"))
    }

    async fn get_image(&self, resp: ESMResp<Image>, uuid: ImageUuid) -> anyhow::Result<()> {
        let inner = async {
            let conn = self.pool.get_conn().await?;

            let result: (String, String, String, String, i32, i32, String, i32, u64, String) = r"
                SELECT (owner, path, size, mtime, x_pixel, y_pixel, visibilty, orientation, date, note) FROM images WHERE uuid = :uuid"
                .with(params! {"uuid" => &uuid})
                .run(conn).await?
                .next().await?
                .map(|row| from_row_opt(row))
                .ok_or_else(|| anyhow::Error::msg(format!("failed to find image {uuid}")))??;

            let output = Image {
                url: String::from(""),
                file: ImageFileData {
                    owner: result.0,
                    path: result.1,
                    size: result.2,
                    mtime: result.3,
                    x_pixel: result.4,
                    y_pixel: result.5,
                },
                metadata: ImageMetadata {
                    visibility: Some(Visibility::from(result.6)),
                    orientation: Some(result.7),
                    date: Some(result.8),
                    note: Some(result.9),
                },
            };

            Ok(output)
        };

        resp.send(inner.await).map_err(|_| anyhow::Error::msg("failed to respond to get_image"))
    }

    async fn update_image(
        &self,
        resp: ESMResp<()>,
        user: String,
        uuid: ImageUuid,
        change: ImageMetadata,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn filter_images(
        &self,
        resp: ESMResp<()>,
        user: String,
        filter: String,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn add_album(&self, resp: ESMResp<()>, album: Album) -> anyhow::Result<()> {
        Ok(())
    }

    async fn get_album(&self, resp: ESMResp<Album>, uuid: AlbumUuid) -> anyhow::Result<()> {
        Ok(())
    }
    async fn update_album(
        &self,
        resp: ESMResp<()>,
        user: String,
        uuid: AlbumUuid,
        change: AlbumMetadata,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn filter_albums(
        &self,
        resp: ESMResp<()>,
        user: String,
        filter: String,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn add_library(&self, resp: ESMResp<()>, library: Library) -> anyhow::Result<()> {
        Ok(())
    }

    async fn get_library(&self, resp: ESMResp<Library>, uuid: LibraryUuid) -> anyhow::Result<()> {
        Ok(())
    }

    async fn update_library(
        &self,
        resp: ESMResp<()>,
        user: String,
        uuid: LibraryUuid,
        change: LibraryMetadata,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

#[async_trait]
impl ESInner for MySQLState {
    fn new() -> Self {
        MySQLState {
            pool: Pool::new(""),
        }
    }

    async fn message_handler(&self, esm: ESM) -> anyhow::Result<()> {
        match esm {
            ESM::Db(message) => match message {
                DbMsg::AddImage { resp, image } => self.add_image(resp, image).await,
                DbMsg::GetImage { resp, uuid } => self.get_image(resp, uuid).await,
                DbMsg::UpdateImage {
                    resp,
                    user,
                    uuid,
                    change,
                } => self.update_image(resp, user, uuid, change).await,
                DbMsg::FilterImages { resp, user, filter } => {
                    self.filter_images(resp, user, filter).await
                }
                DbMsg::AddAlbum { resp, uuid } => self.add_album(resp, uuid).await,
                DbMsg::GetAlbum { resp, uuid } => self.get_album(resp, uuid).await,
                DbMsg::UpdateAlbum {
                    resp,
                    user,
                    uuid,
                    change,
                } => self.update_album(resp, user, uuid, change).await,
                DbMsg::FilterAlbums { resp, user, filter } => {
                    self.filter_albums(resp, user, filter).await
                }
                DbMsg::AddLibrary { resp, library } => self.add_library(resp, library).await,
                DbMsg::GetLibary { resp, uuid } => self.get_library(resp, uuid).await,
                DbMsg::UpdateLibrary {
                    resp,
                    user,
                    uuid,
                    change,
                } => self.update_library(resp, user, uuid, change).await,
            },
            _ => Err(anyhow::Error::msg("not implemented")),
        }
    }
}

pub struct MySQLService {
    config: Arc<ESConfig>,
    sender: ESMSender,
    receiver: Arc<Mutex<ESMReceiver>>,
    handle: AsyncCell<tokio::task::JoinHandle<anyhow::Result<()>>>,
}

#[async_trait]
impl EntanglementService for MySQLService {
    type Inner = MySQLState;

    fn create(config: Arc<ESConfig>) -> (ESMSender, Self) {
        let (tx, rx) = tokio::sync::mpsc::channel::<ESM>(32);

        (
            tx.clone(),
            MySQLService {
                config: config.clone(),
                sender: tx,
                receiver: Arc::new(Mutex::new(rx)),
                handle: AsyncCell::new(),
            },
        )
    }

    async fn start(&self, senders: HashMap<ESM, ESMSender>) -> anyhow::Result<()> {
        // falliable stuff can happen here

        let receiver = Arc::clone(&self.receiver);
        let state = Arc::new(MySQLState::new());

        let serve = {
            async move {
                while let Some(msg) = receiver.lock().await.recv().await {
                    let state = Arc::clone(&state);
                    tokio::task::spawn(async move {
                        match state.message_handler(msg).await {
                            Ok(()) => (),
                            Err(_) => println!("mysql_service failed to reply to message"),
                        }
                    });
                }

                Err::<(), anyhow::Error>(anyhow::Error::msg(format!("channel disconnected")))
            }
        };

        let handle = tokio::task::spawn(serve);

        self.handle.set(handle);

        Ok(())
    }
}
