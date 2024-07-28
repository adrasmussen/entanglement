use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::Context;

use async_cell::sync::AsyncCell;

use async_trait::async_trait;

use mysql_async::{from_row_opt, prelude::*, FromRowError, Pool, Row};

use tokio::sync::Mutex;

use crate::db::{msg::DbMsg, ESDbService};
use crate::service::*;
use api::{image::*, ticket::*, user::*, *};

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
    // add a user to the various tables
    //
    // note that this does not sanity check the library location,
    // and so we should probably standardize that somewhere
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

    // get the details for a particular user, none of which are
    // currently secret or otherwise restricted to just admins
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

    // add an image to the database, providing defaults for the metadata
    // if they are not provided
    //
    // this is intended to be called only by the filesystem service during
    // a library search, so there is no ambiguity on the argument details
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

    // get the data and metadata for a particular image
    //
    // TODO: properly check privs here, which allows for private notes too
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

        let data: (String, String, i64, u32, u32, u32, String, String) = from_row_opt(row)?;

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

    async fn update_media(
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

    // search the database for particular images
    //
    // this is currently used for the gallery view (i.e. single search string) and not,
    // for example, finding all images in a particular album or library
    async fn search_media(
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

    async fn search_albums(&self, user: String, filter: String) -> anyhow::Result<()> {
        todo!()
    }

    // search images in a particular album
    //
    // this is split from the gallery search_images since it will have different behavior
    // w.r.t. joins on the main select
    async fn search_media_in_album(
        &self,
        user: String,
        uuid: AlbumUuid,
        filter: String,
    ) -> anyhow::Result<HashMap<ImageUuid, Image>> {
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

    async fn search_images_in_library(
        &self,
        user: String,
        uuid: LibraryUuid,
        filter: String,
        hidden: bool,
    ) -> anyhow::Result<HashMap<ImageUuid, Image>> {
        todo!()
    }

    // ticket queries
    async fn create_ticket(&self, ticket: Ticket) -> anyhow::Result<TicketUuid> {
        let conn = self
            .pool
            .get_conn()
            .await
            .context("Failed to get MySQL database connection for create_ticket")?;

        let query = r"
            INSERT INTO tickets (uuid, media_uuid, owner, title, timestamp, resolved)
            OUTPUT INSERTED.uuid
            VALUES (UUID_SHORT(), :media_uuid, :owner, :title, :timestamp, :resolved)"
            .with(params! {
                "media_uuid" => ticket.media_uuid,
                "owner" => ticket.owner,
                "title" => ticket.title,
                "timestamp" => ticket.timestamp,
                "resolved" => ticket.resolved,
            });

        let mut result = query
            .run(conn)
            .await
            .context("Failed to run create_ticket query")?;

        let mut rows = result
            .collect::<Row>()
            .await
            .context("Failed to collect create_ticket results")?;

        let row = rows
            .pop()
            .ok_or_else(|| anyhow::Error::msg("Failed to return result for create_ticket"))?;

        let data: TicketUuid =
            from_row_opt(row).context("Failed to convert row for create_ticket")?;

        Ok(data)
    }

    async fn create_comment(
        &self,
        ticket_uuid: TicketUuid,
        comment: TicketComment,
    ) -> anyhow::Result<CommentUuid> {
        let conn = self
            .pool
            .get_conn()
            .await
            .context("Failed to get MySQL database connection for create_comment")?;

        let query = r"
            INSERT INTO comments (uuid, ticket_uuid, owner, text, timestamp)
            OUTPUT INSERTED.uuid
            VALUES (UUID_SHORT(), :ticket_uuid, :owner, :text, :timestamp)"
            .with(params! {
                "ticket_uuid" => ticket_uuid,
                "owner" => comment.owner,
                "text" => comment.text,
                "timestamp" => comment.timestamp,
            });

        let mut result = query
            .run(conn)
            .await
            .context("Failed to run create_comment query")?;

        let mut rows = result
            .collect::<Row>()
            .await
            .context("Failed to collect create_comment results")?;

        let row = rows
            .pop()
            .ok_or_else(|| anyhow::Error::msg("Failed to return result for create_comment"))?;

        let data: CommentUuid =
            from_row_opt(row).context("Failed to convert row for create_comment")?;

        Ok(data)
    }

    async fn get_ticket(&self, ticket_uuid: TicketUuid) -> anyhow::Result<Ticket> {
        let conn = self
            .pool
            .get_conn()
            .await
            .context("Failed to get MySQL database connection for create_comment")?;

        let query = r"
            SELECT (media_uuid, owner, title, timestamp, resolved) FROM tickets WHERE uuid = :uuid;

            SELECT (comment_uuid, owner, text, timestamp) FROM comments WHERE ticket_uuid = :uuid"
            .with(params! {"uuid" => ticket_uuid});

        let mut result = query
            .run(conn)
            .await
            .context("Failed to run get_ticket query")?;

        // first set of results
        let mut ticket_rows = result
            .collect::<Row>()
            .await
            .context("Failed to collect get_ticket ticket results")?;

        let ticket_row = ticket_rows
            .pop()
            .ok_or_else(|| anyhow::Error::msg("Failed to return result for get_ticket"))?;

        let ticket_data: (MediaUuid, String, String, i64, bool) =
            from_row_opt(ticket_row).context("Failed to convert ticket row for get_ticket")?;

        // second set of results
        let mut comment_rows = result
            .collect::<Row>()
            .await
            .context("Failed to collect get_ticket comment results")?;

        let comment_rows = comment_rows
            .into_iter()
            .map(|row| from_row_opt::<(CommentUuid, String, String, i64)>(row))
            .collect::<Result<Vec<_>, FromRowError>>()
            .context("Failed to convert comment row for get_ticket")?;

        let mut comment_data = HashMap::new();

        for row in comment_rows.into_iter() {
            comment_data
                .insert(
                    row.0,
                    TicketComment {
                        owner: row.1,
                        text: row.2,
                        timestamp: row.3,
                    },
                )
                .ok_or_else(|| anyhow::Error::msg("Failed to reassemble comment in get_ticket"))?;
        }

        Ok(Ticket {
            media_uuid: ticket_data.0,
            owner: ticket_data.1,
            title: ticket_data.2,
            timestamp: ticket_data.3,
            resolved: ticket_data.4,
            comments: comment_data,
        })
    }

    async fn search_tickets(
        &self,
        user: String,
        filter: String,
        resolved: bool,
    ) -> anyhow::Result<Vec<TicketUuid>> {
        Err(anyhow::Error::msg("oh no"))
    }
}

#[async_trait]
impl ESInner for MySQLState {
    fn new(
        _config: Arc<ESConfig>,
        _senders: HashMap<ServiceType, ESMSender>,
    ) -> anyhow::Result<Self> {
        Ok(MySQLState {
            pool: Pool::new(""),
        })
    }

    async fn message_handler(&self, esm: ESM) -> anyhow::Result<()> {
        match esm {
            ESM::Db(message) => match message {
                DbMsg::AddUser { resp, user } => self.respond(resp, self.add_user(user)).await,
                DbMsg::GetUser { resp, uid } => self.respond(resp, self.get_user(uid)).await,
                DbMsg::DeleteUser { resp, uid } => self.respond(resp, self.delete_user(uid)).await,
                DbMsg::AddGroup { resp, group } => self.respond(resp, self.add_group(group)).await,
                DbMsg::GetGroup { resp, gid } => self.respond(resp, self.get_group(gid)).await,
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
                    self.respond(resp, self.update_media(user, uuid, change))
                        .await
                }
                DbMsg::SearchImages { resp, user, filter } => {
                    self.respond(resp, self.search_media(user, filter)).await
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
                    self.respond(resp, self.search_albums(user, filter)).await
                }
                DbMsg::SearchImagesInAlbum {
                    resp,
                    user,
                    uuid,
                    filter,
                } => {
                    self.respond(resp, self.search_media_in_album(user, uuid, filter))
                        .await
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
                DbMsg::SearchImagesInLibrary {
                    resp,
                    user,
                    uuid,
                    filter,
                    hidden,
                } => {
                    self.respond(
                        resp,
                        self.search_images_in_library(user, uuid, filter, hidden),
                    )
                    .await
                }

                // ticket messages
                DbMsg::CreateTicket { resp, ticket } => {
                    self.respond(resp, self.create_ticket(ticket)).await
                }
                DbMsg::CreateComment {
                    resp,
                    ticket_uuid,
                    comment,
                } => {
                    self.respond(resp, self.create_comment(ticket_uuid, comment))
                        .await
                }
                DbMsg::GetTicket { resp, ticket_uuid } => {
                    self.respond(resp, self.get_ticket(ticket_uuid)).await
                }
                DbMsg::SearchTickets {
                    resp,
                    user,
                    filter,
                    resolved,
                } => {
                    self.respond(resp, self.search_tickets(user, filter, resolved))
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
