use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_cell::sync::AsyncCell;

use async_trait::async_trait;

use futures::{future::BoxFuture, FutureExt};

use mysql_async::binlog::events::RowsEvent;
use mysql_async::{
    from_row, from_row_opt, prelude::*, BinaryProtocol, FromRowError, Opts, Pool, ResultSetStream,
    Row,
};

use tokio::sync::Mutex;

use crate::db::{msg::DbMsg, ESDbConn, ESDbService};
use crate::service::*;
use api::{auth::*, image::*};

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
    async fn add_user(&self, user: User) -> anyhow::Result<()> {
        let conn = self.pool.get_conn().await?;

        let query = r"
        INSERT INTO users (uid, library, theme)
        OUTPUT INSERTED.uid
        VALUES (:uid, :library, :theme)"
            .with(params! {
                "uid" => user.uid.clone(),
                "library" => user.library,
                "theme" => user.settings.theme.unwrap_or_else(|| String::from("default")),
            });

        query.run(conn).await?;

        for gid in user.groups.iter() {
            self.add_user_to_group(user.uid.clone(), String::from(gid))
                .await?;
        }

        Ok(())
    }

    async fn get_user(&self, uid: String) -> anyhow::Result<User> {
        let conn = self.pool.get_conn().await?;

        let query = r"
        SELECT (uid, library, theme) FROM users WHERE uid = :uid;

        SELECT gid FROM group_members WHERE uid = :uid;"
            .with(params! {
                "uid" => uid.clone(),
            });

        let mut result = query.run(conn).await?;

        // unpack the first resultset
        let user_row = result
            .collect::<Row>()
            .await?
            .pop()
            .ok_or_else(|| anyhow::Error::msg(format! {"failed to find user {uid}"}))?;

        let user_data: (String, String, String) = from_row_opt(user_row)?;

        // unpack the second resultset
        let group_rows = result.collect::<Row>().await?;

        let groups = group_rows
            .into_iter()
            .map(|row| from_row_opt::<String>(row))
            .collect::<Result<HashSet<String>, FromRowError>>()?;

        Ok(User {
            uid: user_data.0,
            groups: groups,
            library: user_data.1,
            settings: UserSettings {
                theme: Some(user_data.2),
            },
        })
    }

    async fn delete_user(&self, uid: String) -> anyhow::Result<()> {
        todo!()
    }

    async fn add_group(&self, group: Group) -> anyhow::Result<()> {
        todo!()
    }

    async fn get_group(&self, gid: String) -> anyhow::Result<Group> {
        todo!()
    }

    async fn delete_group(&self, gid: String) -> anyhow::Result<()> {
        todo!()
    }

    async fn add_user_to_group(&self, uid: String, gid: String) -> anyhow::Result<()> {
        Ok(())
    }

    async fn rm_user_from_group(&self, uid: String, gid: String) -> anyhow::Result<()> {
        todo!()
    }

    async fn add_image(&self, image: Image) -> anyhow::Result<ImageUuid> {
        let conn = self.pool.get_conn().await?;

        let query= r"
                INSERT INTO images (uuid, owner, path, datetime_original, x_pixel, y_pixel, orientation, date, note)
                OUTPUT INSERTED.uuid
                VALUES (UUID_SHORT(), :owner, :path, :datetime_original, :x_pixel, :y_pixel, :orientation, :date, :note)"
                .with(params! {
                    "owner" => image.data.owner,
                    "path" => image.data.path,
                    "datetime_original" => image.data.datetime_original,
                    "x_pixel" => image.data.x_pixel,
                    "y_pixel" => image.data.y_pixel,
                    "orientation" => image.metadata.orientation.unwrap_or_else(|| 0),
                    "data" => image.metadata.date.unwrap_or_else(|| String::from("")),
                    "note" => image.metadata.note.unwrap_or_else(|| String::from("")),
                });

        let mut result = query.run(conn).await?;

        let mut rows = result.collect::<Row>().await?;

        let row = rows.pop().ok_or_else(|| {
            anyhow::Error::msg(format!("failed to return uuid for inserted image"))
        })?;

        let data: ImageUuid = from_row_opt(row)?;

        Ok(data)
    }

    async fn get_image(&self, uuid: ImageUuid) -> anyhow::Result<Image> {
        let conn = self.pool.get_conn().await?;

        let query = r"
                SELECT (owner, path, datetime_original, x_pixel, y_pixel, orientation, date, note) FROM images WHERE uuid = :uuid"
                .with(params! {"uuid" => &uuid});

        let mut result = query.run(conn).await?;

        let mut rows = result.collect::<Row>().await?;

        let row = rows
            .pop()
            .ok_or_else(|| anyhow::Error::msg(format!("failed to find image {uuid}")))?;

        let data: (String, String, u64, i32, i32, i8, String, String) = from_row_opt(row)?;

        Ok(Image {
            data: ImageData {
                owner: data.0,
                path: data.1,
                datetime_original: data.2,
                x_pixel: data.3,
                y_pixel: data.4,
            },
            metadata: ImageMetadata {
                orientation: Some(data.5),
                date: Some(data.6),
                note: Some(data.7),
            },
        })
    }

    async fn update_image(
        &self,
        user: String,
        uuid: ImageUuid,
        change: ImageMetadata,
    ) -> anyhow::Result<()> {
        let conn = self.pool.get_conn().await?;

        /*match change.visibility {
            None => {}
            Some(v) => {self.cache_sender(ESM::Cache(CacheMsg::SetImageVisbiliity))}
        }*/

        let query = r"".with(params! {"" => ""});

        let _result = query.run(conn).await?;

        Ok(())
    }

    async fn filter_images(
        &self,
        user: String,
        filter: String,
    ) -> anyhow::Result<HashMap<ImageUuid, Image>> {
        Err(anyhow::Error::msg(""))
    }

    async fn add_album(&self, album: Album) -> anyhow::Result<()> {
        Err(anyhow::Error::msg(""))
    }

    async fn get_album(&self, uuid: AlbumUuid) -> anyhow::Result<Album> {
        Err(anyhow::Error::msg(""))
    }

    async fn update_album(
        &self,
        user: String,
        uuid: AlbumUuid,
        change: AlbumMetadata,
    ) -> anyhow::Result<()> {
        let conn = self.pool.get_conn().await?;

        let query = r"".with(params! {"" => ""});

        let _result = query.run(conn).await?;

        todo!()
    }

    async fn filter_albums(&self, user: String, filter: String) -> anyhow::Result<()> {
        todo!()
    }

    async fn add_library(&self, library: Library) -> anyhow::Result<()> {
        todo!()
    }

    async fn get_library(&self, uuid: LibraryUuid) -> anyhow::Result<Library> {
        todo!()
    }

    async fn update_library(
        &self,
        user: String,
        uuid: LibraryUuid,
        change: LibraryMetadata,
    ) -> anyhow::Result<()> {
        todo!()
    }
}

#[async_trait]
impl ESInner for MySQLState {
    fn new(_config: Arc<ESConfig>, senders: HashMap<ServiceType, ESMSender>) -> anyhow::Result<Self> {
        Ok(MySQLState {
            pool: Pool::new(""),
        })
    }

    async fn message_handler(&self, esm: ESM) -> anyhow::Result<()> {
        match esm {
            ESM::Db(message) => match message {
                DbMsg::AddUser { resp, user } => {
                    self.respond(resp, self.add_user(user)).await
                }
                DbMsg::GetUser { resp, uid } => {
                    self.respond(resp, self.get_user(uid)).await
                }
                DbMsg::DeleteUser { resp, uid } => {
                    self.respond(resp, self.delete_user(uid)).await
                }
                DbMsg::AddGroup { resp, group } => {
                    self.respond(resp, self.add_group(group)).await
                }
                DbMsg::GetGroup { resp, gid } => {
                    self.respond(resp, self.get_group(gid)).await
                }
                DbMsg::DeleteGroup { resp, gid } => {
                    self.respond(resp, self.delete_group(gid)).await
                }
                DbMsg::AddUserToGroup { resp, uid, gid } => {
                    self.respond(resp, self.add_user_to_group(uid, gid)).await
                }
                DbMsg::RmUserFromGroup { resp, uid, gid } => {
                    self.respond(resp, self.rm_user_from_group(uid, gid)).await
                }
                DbMsg::AddImage { resp, image } => self.respond(resp, self.add_image(image)).await,
                DbMsg::GetImage { resp, user, uuid } => {
                    self.respond(resp, self.get_image(uuid)).await
                }
                DbMsg::UpdateImage {
                    resp,
                    user,
                    uuid,
                    change,
                } => {
                    self.respond(resp, self.update_image(user, uuid, change))
                        .await
                }
                DbMsg::SearchImages { resp, user, filter } => {
                    self.respond(resp, self.filter_images(user, filter)).await
                }
                DbMsg::GetImageGroups { resp, uuid } => {
                    todo!()
                }
                DbMsg::AddAlbum { resp, user, album } => {
                    self.respond(resp, self.add_album(album)).await
                }
                DbMsg::GetAlbum { resp, user, uuid } => {
                    self.respond(resp, self.get_album(uuid)).await
                }
                DbMsg::DeleteAlbum { resp, user, uuid } => {
                    todo!()
                }
                DbMsg::UpdateAlbum {
                    resp,
                    user,
                    uuid,
                    change,
                } => {
                    self.respond(resp, self.update_album(user, uuid, change))
                        .await
                }
                DbMsg::SearchAlbums { resp, user, filter } => {
                    self.respond(resp, self.filter_albums(user, filter)).await
                }
                DbMsg::AddLibrary { resp, library } => {
                    self.respond(resp, self.add_library(library)).await
                }
                DbMsg::GetLibary { resp, uuid } => self.respond(resp, self.get_library(uuid)).await,
                DbMsg::UpdateLibrary {
                    resp,
                    user,
                    uuid,
                    change,
                } => {
                    self.respond(resp, self.update_library(user, uuid, change))
                        .await
                }
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

    async fn start(&self, senders: HashMap<ServiceType, ESMSender>) -> anyhow::Result<()> {
        // falliable stuff can happen here

        let receiver = Arc::clone(&self.receiver);
        let state = Arc::new(MySQLState::new(self.config.clone(), senders)?);

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
