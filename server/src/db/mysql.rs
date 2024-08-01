use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::Context;

use async_cell::sync::AsyncCell;

use async_trait::async_trait;

use mysql_async::{from_row_opt, prelude::*, FromRowError, Pool, Row};

use tokio::sync::Mutex;

use crate::db::{msg::DbMsg, ESDbService};
use crate::service::*;
use api::{album::*, group::*, image::*, library::*, ticket::*, user::*, *};

pub struct MySQLState {
    pool: Pool,
}

// database RPC handler functions
#[async_trait]
impl ESDbService for MySQLState {
    // auth queries
    async fn can_access_media(&self, uid: String, media_uuid: MediaUuid) -> anyhow::Result<bool> {
        todo!()
    }
    // add a user to the various tables
    //
    // note that this does not sanity check the library location,
    // and so we should probably standardize that somewhere
    async fn add_user(&self, user: User) -> anyhow::Result<()> {
        todo!()
    }

    // get the details for a particular user, none of which are
    // currently secret or otherwise restricted to just admins
    async fn get_user(&self, uid: String) -> anyhow::Result<User> {
        todo!()
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

    async fn add_media(&self, media: Media) -> anyhow::Result<MediaUuid> {
        todo!()
    }

    async fn get_media(&self, media_uuid: MediaUuid) -> anyhow::Result<Media> {
        todo!()
    }

    async fn update_media(
        &self,
        media_uuid: MediaUuid,
        change: MediaMetadata,
    ) -> anyhow::Result<()> {
        let conn = self
            .pool
            .get_conn()
            .await
            .context("Failed to get MySQL database connection for update_media")?;

        let query = r"
            UPDATE media SET hidden = :hidden, date = :date, note = :note WHERE media_uuid = :media_uuid"
            .with(params! {
                "hidden" => change.hidden,
                "date" => change.date,
                "note" => change.note,
                "media_uuid" => media_uuid,
            });

        query
            .run(conn)
            .await
            .context("Failed to run update_media query")?;

        Ok(())
    }

    async fn search_media(&self, user: String, filter: String) -> anyhow::Result<Vec<MediaUuid>> {
        let conn = self
            .pool
            .get_conn()
            .await
            .context("Failed to get MySQL database connection for search_media")?;

        let query = r"
            (SELECT gid FROM ((SELECT uid FROM users WHERE uid = :user) INNER JOIN group_membership)) as user_gids
            (SELECT album_uuid FROM (user_gids INNER JOIN albums ON user_gids.gid = albums.group) as user_albums
            (SELECT media_uuid FROM (user_albums INNER JOIN album_contents ON user_albums.album_uuid = album_contents.album_uuid) as user_media

            SELECT media_uuid FROM (user_media INNER JOIN media ON user_media.media_uuid = media.media_uuid)
                WHERE (filter LIKE CONCAT_WS(' ', date, note))"
            .with(params! {
                "user" => user,
                "filter" => filter,
            });

        let mut result = query
            .run(conn)
            .await
            .context("Failed to run search_media query")?;

        let rows = result
            .collect::<Row>()
            .await
            .context("Failed to collect search_media results")?;

        let data = rows
            .into_iter()
            .map(|row| from_row_opt::<MediaUuid>(row))
            .collect::<Result<Vec<_>, FromRowError>>()
            .context("Failed to convert uuid row for search_media")?;

        Ok(data)
    }

    // album queries
    async fn add_album(&self, album: Album) -> anyhow::Result<AlbumUuid> {
        let conn = self
            .pool
            .get_conn()
            .await
            .context("Failed to get MySQL database connection for add_album")?;

        let query = r"
            INSERT INTO album (album_uuid, owner, group, name, note)
            OUTPUT INSERTED.album_uuid
            VALUES (UUID_SHORT(), :owner, :group, :name, :note)"
            .with(params! {
                "owner" => album.owner,
                "group" => album.group,
                "name" => album.metadata.name,
                "note" => album.metadata.note,
            });

        let mut result = query
            .run(conn)
            .await
            .context("Failed to run add_album query")?;

        let mut rows = result
            .collect::<Row>()
            .await
            .context("Failed to collect add_album results")?;

        let row = rows
            .pop()
            .ok_or_else(|| anyhow::Error::msg("Failed to return result for add_album"))?;

        let data: AlbumUuid = from_row_opt(row).context("Failed to convert row for add_album")?;

        Ok(data)
    }

    async fn get_album(&self, album_uuid: AlbumUuid) -> anyhow::Result<Album> {
        let conn = self
            .pool
            .get_conn()
            .await
            .context("Failed to get MySQL database connection for get_album")?;

        let query = r"
            SELECT (owner, group, name, note) FROM albums WHERE album_uuid = :album_uuid"
            .with(params! {
                "album_uuid" => album_uuid,
            });

        let mut result = query
            .run(conn)
            .await
            .context("Failed to run get_album query")?;

        let mut rows = result
            .collect::<Row>()
            .await
            .context("Failed to collect get_album results")?;

        let row = rows
            .pop()
            .ok_or_else(|| anyhow::Error::msg("Failed to return result for get_album"))?;

        let data: (String, String, String, String) =
            from_row_opt(row).context("Failed to convert row for get_album")?;

        Ok(Album {
            owner: data.0,
            group: data.1,
            metadata: AlbumMetadata {
                name: data.2,
                note: data.3,
            },
        })
    }

    async fn delete_album(&self, album_uuid: AlbumUuid) -> anyhow::Result<()> {
        let conn = self
            .pool
            .get_conn()
            .await
            .context("Failed to get MySQL database connection for delete_album")?;

        let query = r"
            DELETE FROM albums WHERE album_uuid = :album_uuid"
            .with(params! {
                "album_uuid" => album_uuid,
            });

        query
            .run(conn)
            .await
            .context("Failed to run delete_album query")?;

        Ok(())
    }

    async fn update_album(
        &self,
        album_uuid: AlbumUuid,
        change: AlbumMetadata,
    ) -> anyhow::Result<()> {
        let conn = self
            .pool
            .get_conn()
            .await
            .context("Failed to get MySQL database connection for update_album")?;

        let query = r"
            UPDATE albums SET name = :name, note = :note WHERE album_uuid = :album_uuid"
            .with(params! {
                "name" => change.name,
                "note" => change.note,
                "album_uuid" => album_uuid,
            });

        query
            .run(conn)
            .await
            .context("Failed to run update_album query")?;

        Ok(())
    }

    async fn add_media_to_album(
        &self,
        media_uuid: MediaUuid,
        album_uuid: AlbumUuid,
    ) -> anyhow::Result<()> {
        let conn = self
            .pool
            .get_conn()
            .await
            .context("Failed to get MySQL database connection for add_media_to_album")?;

        let query = r"
            INSERT INTO album_contents (media_uuid, album_uuid)
            VALUES (:media_uuid, :album_uuid)"
            .with(params! {
                "media_uuid" => media_uuid,
                "album_uuid" => album_uuid,
            });

        query
            .run(conn)
            .await
            .context("Failed to run add_media_to_album query")?;

        Ok(())
    }

    async fn rm_media_from_album(
        &self,
        media_uuid: MediaUuid,
        album_uuid: AlbumUuid,
    ) -> anyhow::Result<()> {
        let conn = self
            .pool
            .get_conn()
            .await
            .context("Failed to get MySQL database connection for rm_media_from_album")?;

        let query = r"
            DELETE FROM album_contents WHERE (media_uuid = :media_uuid AND album_uuid = :album_uuid)"
            .with(params!{
                "media_uuid" => media_uuid,
                "album_uuid" => album_uuid,
            });

        query
            .run(conn)
            .await
            .context("Failed to run rm_media_from_album query")?;

        Ok(())
    }

    async fn search_albums(&self, user: String, filter: String) -> anyhow::Result<Vec<AlbumUuid>> {
        let conn = self
            .pool
            .get_conn()
            .await
            .context("Failed to get MySQL database connection for search_albums")?;

        let query = r"
            (SELECT gid FROM ((SELECT uid FROM users WHERE uid = :user) INNER JOIN group_membership)) as user_gids
            SELECT album_uuid FROM (user_gids INNER JOIN albums ON user_gids.gid = albums.group)
                WHERE (filter LIKE CONCAT_WS(' ', name, note)"
            .with(params! {
                "user" => user,
                "filter" => filter,
            });

        let mut result = query
            .run(conn)
            .await
            .context("Failed to run search_albums query")?;

        let rows = result
            .collect::<Row>()
            .await
            .context("Failed to collect search_albums results")?;

        let data = rows
            .into_iter()
            .map(|row| from_row_opt::<AlbumUuid>(row))
            .collect::<Result<Vec<_>, FromRowError>>()
            .context("Failed to convert uuid row for search_albums")?;

        Ok(data)
    }

    async fn search_media_in_album(
        &self,
        user: String,
        album_uuid: AlbumUuid,
        filter: String,
    ) -> anyhow::Result<Vec<MediaUuid>> {
        let conn = self
            .pool
            .get_conn()
            .await
            .context("Failed to get MySQL database connection for search_media_in_album")?;

        let query = r"
            (SELECT gid FROM ((SELECT uid FROM users WHERE uid = :user) INNER JOIN group_membership)) as user_gids
            (SELECT :album_uuid FROM (user_gids INNER JOIN albums ON user_gids.gid = albums.group) as user_albums
            (SELECT media_uuid FROM (user_albums INNER JOIN album_contents ON user_albums.album_uuid = album_contents.album_uuid) as user_media

            SELECT media_uuid FROM (user_media INNER JOIN media ON user_media.media_uuid = media.media_uuid)
                WHERE (filter LIKE CONCAT_WS(' ', date, note))"
            .with(params! {
                "user" => user,
                "album_uuid" => album_uuid,
                "filter" => filter,
            });

        let mut result = query
            .run(conn)
            .await
            .context("Failed to run search_media_in_album query")?;

        let rows = result
            .collect::<Row>()
            .await
            .context("Failed to collect search_media_in_album results")?;

        let data = rows
            .into_iter()
            .map(|row| from_row_opt::<MediaUuid>(row))
            .collect::<Result<Vec<_>, FromRowError>>()
            .context("Failed to convert uuid row for search_media_in_album")?;

        Ok(data)
    }

    // library queries
    async fn add_library(&self, library: Library) -> anyhow::Result<LibraryUuid> {
        let conn = self
            .pool
            .get_conn()
            .await
            .context("Failed to get MySQL database connection for add_library")?;

        let query = r"
            INSERT INTO libraries (library_uuid, path, owner, group, file_count, last_scan)
            OUTPUT INSERTED.library_uuid
            VALUES (UUID_SHORT(), :path, :owner, :group, :file_count, :last_scan)"
            .with(params! {
                "path" => library.path,
                "owner" => library.owner,
                "group" => library.group,
                "file_count" => library.file_count,
                "last_scan" => library.last_scan
            });

        let mut result = query
            .run(conn)
            .await
            .context("Failed to run add_library query")?;

        let mut rows = result
            .collect::<Row>()
            .await
            .context("Failed to collect add_library results")?;

        let row = rows
            .pop()
            .ok_or_else(|| anyhow::Error::msg("Failed to return result for add_library"))?;

        let data: LibraryUuid =
            from_row_opt(row).context("Failed to convert row for add_library")?;

        Ok(data)
    }

    async fn get_library(&self, library_uuid: LibraryUuid) -> anyhow::Result<Library> {
        let conn = self
            .pool
            .get_conn()
            .await
            .context("Failed to get MySQL database connection for get_library")?;

        let query = r"
            SELECT (path, owner, group, file_count, last_scan) FROM libraries WHERE library_uuid = :library_uuid"
            .with(params! {
                "library_uuid" => library_uuid,
            });

        let mut result = query
            .run(conn)
            .await
            .context("Failed to run get_library query")?;

        let mut rows = result
            .collect::<Row>()
            .await
            .context("Failed to collect get_library results")?;

        let row = rows
            .pop()
            .ok_or_else(|| anyhow::Error::msg("Failed to return result for get_library"))?;

        let data: (String, String, String, i64, i64) =
            from_row_opt(row).context("Failed to convert row for get_library")?;

        Ok(Library {
            path: data.0,
            owner: data.1,
            group: data.2,
            file_count: data.3,
            last_scan: data.4,
        })
    }

    async fn search_media_in_library(
        &self,
        user: String,
        library_uuid: LibraryUuid,
        filter: String,
        hidden: bool,
    ) -> anyhow::Result<Vec<MediaUuid>> {
        let conn = self
            .pool
            .get_conn()
            .await
            .context("Failed to get MySQL database connection for search_media_in_library")?;

        let query = r"
            (SELECT gid FROM ((SELECT uid FROM users WHERE uid = :user) INNER JOIN group_membership)) as user_gids
            (SELECT album_uuid FROM (user_gids INNER JOIN albums ON user_gids.gid = albums.group) as user_albums
            (SELECT media_uuid FROM (user_albums INNER JOIN album_contents ON user_albums.album_uuid = album_contents.album_uuid) as user_media

            SELECT media_uuid FROM (user_media INNER JOIN media ON user_media.media_uuid = media.media_uuid)
                WHERE (library = :library_uuid)
                    AND (hidden = :hidden)
                    AND (filter LIKE CONCAT_WS(' ', date, note))"
            .with(params! {
                "user" => user,
                "library_uuid" => library_uuid,
                "hidden" => hidden,
                "filter" => filter,
            });

        let mut result = query
            .run(conn)
            .await
            .context("Failed to run search_media_in_library query")?;

        let rows = result
            .collect::<Row>()
            .await
            .context("Failed to collect search_media_in_library results")?;

        let data = rows
            .into_iter()
            .map(|row| from_row_opt::<MediaUuid>(row))
            .collect::<Result<Vec<_>, FromRowError>>()
            .context("Failed to convert uuid row for search_media_in_library")?;

        Ok(data)
    }

    // ticket queries
    async fn create_ticket(&self, ticket: Ticket) -> anyhow::Result<TicketUuid> {
        let conn = self
            .pool
            .get_conn()
            .await
            .context("Failed to get MySQL database connection for create_ticket")?;

        let query = r"
            INSERT INTO tickets (ticket_uuid, media_uuid, owner, title, timestamp, resolved)
            OUTPUT INSERTED.ticket_uuid
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

    async fn create_comment(&self, comment: TicketComment) -> anyhow::Result<CommentUuid> {
        let conn = self
            .pool
            .get_conn()
            .await
            .context("Failed to get MySQL database connection for create_comment")?;

        let query = r"
            INSERT INTO comments (comment_uuid, ticket_uuid, owner, text, timestamp)
            OUTPUT INSERTED.comment_uuid
            VALUES (UUID_SHORT(), :ticket_uuid, :owner, :text, :timestamp)"
            .with(params! {
                "ticket_uuid" => comment.ticket_uuid,
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
            SELECT (media_uuid, owner, title, timestamp, resolved) FROM tickets WHERE ticket_uuid = :ticket_uuid;

            SELECT (comment_uuid, owner, text, timestamp) FROM comments WHERE ticket_uuid = :ticket_uuid"
            .with(params! {"ticket_uuid" => ticket_uuid});

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
        let comment_rows = result
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
            match comment_data.insert(
                row.0,
                TicketComment {
                    ticket_uuid: ticket_uuid,
                    owner: row.1,
                    text: row.2,
                    timestamp: row.3,
                },
            ) {
                None => {}
                Some(_) => {
                    return Err(anyhow::Error::msg(
                        "Failed to assemble ticket comments due to duplicate comment uuid",
                    ))
                }
            }
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
        let conn = self
            .pool
            .get_conn()
            .await
            .context("Failed to get MySQL database connection for search_tickets")?;

        let query = r"
            (SELECT gid FROM ((SELECT uid FROM users WHERE uid = :user) INNER JOIN group_membership)) as user_gids
            (SELECT uuid FROM (user_gids INNER JOIN albums ON user_gids.gid = albums.group) as user_albums
            (SELECT media_uuid FROM (user_albums INNER JOIN album_contents ON user_albums.uuid = album_contents.album_uuid) as user_media

            SELECT ticket_uuid FROM (user_media INNER JOIN tickets ON user_media.media_uuid = tickets.media_uuid)
                WHERE (resolved = :resolved)
                    AND (filter LIKE text)"
            .with(params! {
                "user" => user,
                "resolved" => resolved,
                "filter" => filter,
            });

        let mut result = query
            .run(conn)
            .await
            .context("Failed to run search_tickets query")?;

        let rows = result
            .collect::<Row>()
            .await
            .context("Failed to collect search_tickets results")?;

        let data = rows
            .into_iter()
            .map(|row| from_row_opt::<TicketUuid>(row))
            .collect::<Result<Vec<_>, FromRowError>>()
            .context("Failed to convert uuid row for search_tickets")?;

        Ok(data)
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
                // auth messages
                DbMsg::CanAccessMedia {
                    resp,
                    uid,
                    media_uuid,
                } => {
                    self.respond(resp, self.can_access_media(uid, media_uuid))
                        .await
                }
                DbMsg::AddUser { resp, user } => self.respond(resp, self.add_user(user)).await,
                DbMsg::GetUser { resp, uid } => self.respond(resp, self.get_user(uid)).await,
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
                DbMsg::AddMedia { resp, media } => self.respond(resp, self.add_media(media)).await,
                DbMsg::GetMedia { resp, media_uuid } => {
                    self.respond(resp, self.get_media(media_uuid)).await
                }
                DbMsg::UpdateMedia {
                    resp,
                    media_uuid,
                    change,
                } => {
                    self.respond(resp, self.update_media(media_uuid, change))
                        .await
                }
                DbMsg::SearchMedia { resp, user, filter } => {
                    self.respond(resp, self.search_media(user, filter)).await
                }

                // album messages
                DbMsg::AddAlbum { resp, album } => self.respond(resp, self.add_album(album)).await,
                DbMsg::GetAlbum { resp, album_uuid } => {
                    self.respond(resp, self.get_album(album_uuid)).await
                }
                DbMsg::DeleteAlbum { resp, album_uuid } => {
                    self.respond(resp, self.delete_album(album_uuid)).await
                }
                DbMsg::UpdateAlbum {
                    resp,
                    album_uuid,
                    change,
                } => {
                    self.respond(resp, self.update_album(album_uuid, change))
                        .await
                }
                DbMsg::AddMediaToAlbum {
                    resp,
                    media_uuid,
                    album_uuid,
                } => {
                    self.respond(resp, self.add_media_to_album(media_uuid, album_uuid))
                        .await
                }
                DbMsg::RmMediaFromAlbum {
                    resp,
                    media_uuid,
                    album_uuid,
                } => {
                    self.respond(resp, self.rm_media_from_album(media_uuid, album_uuid))
                        .await
                }
                DbMsg::SearchAlbums { resp, user, filter } => {
                    self.respond(resp, self.search_albums(user, filter)).await
                }
                DbMsg::SearchMediaInAlbum {
                    resp,
                    user,
                    album_uuid,
                    filter,
                } => {
                    self.respond(resp, self.search_media_in_album(user, album_uuid, filter))
                        .await
                }

                // library messages
                DbMsg::AddLibrary { resp, library } => {
                    self.respond(resp, self.add_library(library)).await
                }
                DbMsg::GetLibary { resp, uuid } => self.respond(resp, self.get_library(uuid)).await,
                DbMsg::SearchMediaInLibrary {
                    resp,
                    user,
                    uuid,
                    filter,
                    hidden,
                } => {
                    self.respond(
                        resp,
                        self.search_media_in_library(user, uuid, filter, hidden),
                    )
                    .await
                }

                // ticket messages
                DbMsg::CreateTicket { resp, ticket } => {
                    self.respond(resp, self.create_ticket(ticket)).await
                }
                DbMsg::CreateComment { resp, comment } => {
                    self.respond(resp, self.create_comment(comment)).await
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
