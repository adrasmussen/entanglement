use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_cell::sync::AsyncCell;

use async_trait::async_trait;

use mysql_async::{from_row_opt, prelude::*, FromRowError, Pool, Row};

use tokio::sync::Mutex;

use crate::auth::msg::*;
use crate::db::{msg::DbMsg, ESDbService};
use crate::service::*;
use api::{album::*, group::*, library::*, media::*, ticket::*, user::*};

pub struct MySQLState {
    auth_svc_sender: ESMSender,
    pool: Pool,
}

impl MySQLState {
    async fn clear_access_cache(&self, media_uuid: Vec<MediaUuid>) -> anyhow::Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.auth_svc_sender
            .send(
                AuthMsg::ClearAccessCache {
                    resp: tx,
                    uuid: media_uuid,
                }
                .into(),
            )
            .await?;

        rx.await?
    }

    async fn clear_user_cache(&self, uid: Vec<String>) -> anyhow::Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.auth_svc_sender
            .send(AuthMsg::ClearUserCache { resp: tx, uid: uid }.into())
            .await?;

        rx.await?
    }
}

// database RPC handler functions
#[async_trait]
impl ESDbService for MySQLState {
    // auth queries
    async fn media_access_groups(&self, media_uuid: MediaUuid) -> anyhow::Result<HashSet<String>> {
        // for a given media_uuid, find all gids that match either:
        //  * if the media is not hidden, any album that contains the media
        //  * the library that contains that media
        let result = r"
            SELECT
                gid
            FROM
                (
                SELECT
                    album_uuid
                FROM
                    media
                INNER JOIN album_contents ON media.media_uuid = album_contents.media_uuid
                WHERE
                    media.media_uuid = :media_uuid AND media.hidden = FALSE
            ) AS t1
            INNER JOIN albums ON t1.album_uuid = albums.album_uuid
            UNION
            SELECT
                gid
            FROM
                (
                    libraries
                INNER JOIN media ON libraries.library_uuid = media.media_uuid
                )
            WHERE
                media_uuid = :media_uuid"
            .with(params! {
                "media_uuid" => media_uuid,
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let data = result
            .into_iter()
            .map(|row| from_row_opt::<String>(row))
            .collect::<Result<HashSet<_>, FromRowError>>()?;

        Ok(data)
    }

    // user queries
    async fn create_user(&self, uid: String, _metadata: UserMetadata) -> anyhow::Result<()> {
        r"
            INSERT INTO users (uid)
            VALUES (:uid)"
            .with(params! {"uid" => uid.clone()})
            .run(self.pool.get_conn().await?)
            .await?;

        r"
            INSERT INTO groups (gid)
            VALUES (:uid)"
            .with(params! {"uid" => uid.clone()})
            .run(self.pool.get_conn().await?)
            .await?;

        r"
            INSERT INTO group_membership (uid, gid)
            VALUES (:uid, :uid)"
            .with(params! {"uid" => uid.clone()})
            .run(self.pool.get_conn().await?)
            .await?;

        Ok(())
    }

    async fn get_user(&self, uid: String) -> anyhow::Result<Option<User>> {
        let mut user_result = r"
            SELECT uid FROM users WHERE uid = :uid"
            .with(params! {"uid" => uid.clone()})
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let user_row = match user_result.pop() {
            Some(row) => row,
            None => return Ok(None),
        };

        let user_data = from_row_opt::<String>(user_row)?;

        let group_result = r"
            SELECT gid FROM group_membership WHERE uid = :uid"
            .with(params! {"uid" => uid})
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let group_data = group_result
            .into_iter()
            .map(|row| from_row_opt::<String>(row))
            .collect::<Result<HashSet<_>, FromRowError>>()?;

        Ok(Some(User {
            uid: user_data,
            metadata: UserMetadata {},
            groups: group_data,
        }))
    }

    async fn delete_user(&self, uid: String) -> anyhow::Result<()> {
        r"
            DELETE FROM users WHERE uid = :uid"
            .with(params! {
                "uid" => uid.clone(),
            })
            .run(self.pool.get_conn().await?)
            .await?;

        r"
            DELETE FROM group_membership WHERE uid = :uid"
            .with(params! {
                "uid" => uid.clone(),
            })
            .run(self.pool.get_conn().await?)
            .await?;

        self.clear_user_cache(Vec::from([uid])).await?;

        Ok(())
    }

    // group queries
    async fn create_group(&self, gid: String, _metadata: GroupMetadata) -> anyhow::Result<()> {
        r"
            INSERT INTO groups (gid)
            VALUES (:gid)"
            .with(params! {
                "gid" => gid,
            })
            .run(self.pool.get_conn().await?)
            .await?;

        Ok(())
    }

    async fn get_group(&self, gid: String) -> anyhow::Result<Option<Group>> {
        let mut group_rows = r"
            SELECT gid FROM groups WHERE gid = :gid"
            .with(params! {"gid" => gid.clone()})
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let group_row = match group_rows.pop() {
            Some(row) => row,
            None => return Ok(None),
        };

        let group_data = from_row_opt::<String>(group_row)?;

        let user_rows = r"
            SELECT uid FROM group_membership WHERE gid = :gid"
            .with(params! {"gid" => gid})
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let user_data = user_rows
            .into_iter()
            .map(|row| from_row_opt::<String>(row))
            .collect::<Result<HashSet<_>, FromRowError>>()?;

        Ok(Some(Group {
            gid: group_data,
            metadata: GroupMetadata {},
            members: user_data,
        }))
    }

    async fn delete_group(&self, gid: String) -> anyhow::Result<()> {
        r"
            DELETE FROM users WHERE gid = :gid"
            .with(params! {
                "gid" => gid.clone(),
            })
            .run(self.pool.get_conn().await?)
            .await?;

        r"
            DELETE FROM group_membership WHERE gid = :gid"
            .with(params! {
                "gid" => gid,
            })
            .run(self.pool.get_conn().await?)
            .await?;

        self.clear_user_cache(Vec::new()).await?;

        Ok(())
    }

    async fn add_user_to_group(&self, uid: String, gid: String) -> anyhow::Result<()> {
        r"
            INSERT INTO group_membership (uid, gid)
            VALUES (:uid, :gid)"
            .with(params! {
                "uid" => uid.clone(),
                "gid" => gid,
            })
            .run(self.pool.get_conn().await?)
            .await?;

        self.clear_user_cache(Vec::from([uid])).await?;

        Ok(())
    }

    async fn rm_user_from_group(&self, uid: String, gid: String) -> anyhow::Result<()> {
        r"
            DELETE FROM group_membership WHERE (uid = :uid AND gid = :gid)
            VALUES (:uid, :gid)"
            .with(params! {
                "uid" => uid.clone(),
                "gid" => gid,
            })
            .run(self.pool.get_conn().await?)
            .await?;

        self.clear_user_cache(Vec::from([uid])).await?;

        Ok(())
    }

    // media queries
    async fn add_media(&self, media: Media) -> anyhow::Result<MediaUuid> {
        let mut result = r"
            INSERT INTO media (media_uuid, library_uuid, path, hidden, date, note)
            OUTPUT INSERTED.media_uuid
            VALUES (UUID_SHORT(), :library_uuid, :path, :hidden, :date, :note)"
            .with(params! {
                "library_uuid" => media.library_uuid,
                "path" => media.path,
                "hidden" => media.hidden,
                "date" => media.metadata.date,
                "note" => media.metadata.note,
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let row = result
            .pop()
            .ok_or_else(|| anyhow::Error::msg("Failed to return result for add_media"))?;

        let data = from_row_opt::<MediaUuid>(row)?;

        Ok(data)
    }

    async fn get_media(&self, media_uuid: MediaUuid) -> anyhow::Result<Option<Media>> {
        let mut result = r"
            SELECT library_uuid, path, hidden, date, note FROM media WHERE media_uuid = :media_uuid"
            .with(params! {
                "media_uuid" => media_uuid,
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let row = match result.pop() {
            Some(row) => row,
            None => return Ok(None),
        };

        let data = from_row_opt::<(LibraryUuid, String, bool, String, String)>(row)?;

        Ok(Some(Media {
            library_uuid: data.0,
            path: data.1,
            hidden: data.2,
            metadata: MediaMetadata {
                date: data.3,
                note: data.4,
            },
        }))
    }

    async fn get_media_uuid_by_path(&self, path: String) -> anyhow::Result<Option<MediaUuid>> {
        let mut result = r"
            SELECT media_uuid FROM media WHERE path = :path"
            .with(params! {
                "path" => path,
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let row = match result.pop() {
            Some(row) => row,
            None => return Ok(None),
        };

        let data = from_row_opt::<MediaUuid>(row)?;

        Ok(Some(data))
    }

    async fn update_media(
        &self,
        media_uuid: MediaUuid,
        change: MediaMetadata,
    ) -> anyhow::Result<()> {
        r"
            UPDATE media SET date = :date, note = :note WHERE media_uuid = :media_uuid"
            .with(params! {
                "date" => change.date,
                "note" => change.note,
                "media_uuid" => media_uuid,
            })
            .run(self.pool.get_conn().await?)
            .await?;

        Ok(())
    }

    async fn set_media_hidden(&self, media_uuid: MediaUuid, hidden: bool) -> anyhow::Result<()> {
        r"
            UPDATE media SET hidden = :hidden WHERE media_uuid = :media_uuid"
            .with(params! {
                "hidden" => hidden,
                "media_uuid" => media_uuid,
            })
            .run(self.pool.get_conn().await?)
            .await?;

        self.clear_access_cache(Vec::from([media_uuid.clone()]))
            .await?;

        Ok(())
    }

    async fn search_media(&self, uid: String, filter: String) -> anyhow::Result<Vec<MediaUuid>> {
        // for a given uid and filter, find all media that match either:
        //  * is in a library owned by a group containing the uid
        //  * if the media is not hidden, is in an album owned
        //    by a group containing the uid
        let result = r"
            SELECT
                media.media_uuid
            FROM
                (
                SELECT
                    media_uuid
                FROM
                    (
                    SELECT
                        album_uuid
                    FROM
                        (
                        SELECT
                            gid
                        FROM
                            group_membership
                        WHERE
                            uid = :uid
                    ) AS t1
                INNER JOIN albums ON t1.gid = albums.gid
                ) AS t2
            INNER JOIN album_contents ON t2.album_uuid = album_contents.album_uuid
            UNION
            SELECT
                media_uuid
            FROM
                (
                SELECT
                    library_uuid
                FROM
                    (
                    SELECT
                        gid
                    FROM
                        group_membership
                    WHERE
                        uid = :uid
                ) AS t1
            INNER JOIN libraries ON t1.gid = libraries.gid
            ) AS t2
            INNER JOIN media ON t2.library_uuid = media.library_uuid
            ) AS t3
            INNER JOIN media ON t3.media_uuid = media.media_uuid
            WHERE
                hidden = FALSE AND CONCAT(' ', date, note) LIKE :filter"
            .with(params! {
                "uid" => uid,
                "filter" => format!("%{}%", filter),
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let data = result
            .into_iter()
            .map(|row| from_row_opt::<MediaUuid>(row))
            .collect::<Result<Vec<_>, FromRowError>>()?;

        Ok(data)
    }

    // album queries
    async fn create_album(&self, album: Album) -> anyhow::Result<AlbumUuid> {
        let mut result = r"
            INSERT INTO album (album_uuid, uid, gid, name, note)
            OUTPUT INSERTED.album_uuid
            VALUES (UUID_SHORT(), :uid, :gid, :name, :note)"
            .with(params! {
                "uid" => album.uid,
                "gid" => album.gid,
                "name" => album.metadata.name,
                "note" => album.metadata.note,
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let row = result
            .pop()
            .ok_or_else(|| anyhow::Error::msg("Failed to return result for create_album"))?;

        let data = from_row_opt::<AlbumUuid>(row)?;

        Ok(data)
    }

    async fn get_album(&self, album_uuid: AlbumUuid) -> anyhow::Result<Option<Album>> {
        let mut result = r"
            SELECT uid, gid, name, note FROM albums WHERE album_uuid = :album_uuid"
            .with(params! {
                "album_uuid" => album_uuid,
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let row = match result.pop() {
            Some(row) => row,
            None => return Ok(None),
        };

        let data = from_row_opt::<(String, String, String, String)>(row)?;

        Ok(Some(Album {
            uid: data.0,
            gid: data.1,
            metadata: AlbumMetadata {
                name: data.2,
                note: data.3,
            },
        }))
    }

    async fn delete_album(&self, album_uuid: AlbumUuid) -> anyhow::Result<()> {
        let result = r"
            DELETE FROM album_contents WHERE album_uuid = :album_uuid;
            OUTPUT DELETED.media_uuid"
            .with(params! {
                "album_uuid" => album_uuid,
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        r"
            DELETE FROM albums WHERE album_uuid = :album_uuid;"
            .with(params! {
                "album_uuid" => album_uuid,
            })
            .run(self.pool.get_conn().await?)
            .await?;

        let deleted_media_data = result
            .into_iter()
            .map(|row| from_row_opt::<MediaUuid>(row))
            .collect::<Result<Vec<_>, FromRowError>>()?;

        self.clear_access_cache(deleted_media_data).await?;

        Ok(())
    }

    async fn update_album(
        &self,
        album_uuid: AlbumUuid,
        change: AlbumMetadata,
    ) -> anyhow::Result<()> {
        r"
            UPDATE albums SET name = :name, note = :note WHERE album_uuid = :album_uuid"
            .with(params! {
                "name" => change.name,
                "note" => change.note,
                "album_uuid" => album_uuid,
            })
            .run(self.pool.get_conn().await?)
            .await?;

        Ok(())
    }

    async fn add_media_to_album(
        &self,
        media_uuid: MediaUuid,
        album_uuid: AlbumUuid,
    ) -> anyhow::Result<()> {
        r"
            INSERT INTO album_contents (media_uuid, album_uuid)
            VALUES (:media_uuid, :album_uuid)"
            .with(params! {
                "media_uuid" => media_uuid,
                "album_uuid" => album_uuid,
            })
            .run(self.pool.get_conn().await?)
            .await?;

        self.clear_access_cache(Vec::from([media_uuid.clone()]))
            .await?;

        Ok(())
    }

    async fn rm_media_from_album(
        &self,
        media_uuid: MediaUuid,
        album_uuid: AlbumUuid,
    ) -> anyhow::Result<()> {
        r"
            DELETE FROM album_contents WHERE (media_uuid = :media_uuid AND album_uuid = :album_uuid)"
            .with(params!{
                "media_uuid" => media_uuid,
                "album_uuid" => album_uuid,
            })
            .run(self.pool.get_conn().await?)
            .await?;

        self.clear_access_cache(Vec::from([media_uuid.clone()]))
            .await?;

        Ok(())
    }

    async fn search_albums(&self, uid: String, filter: String) -> anyhow::Result<Vec<AlbumUuid>> {
        // for a given uid and filter, find all albums owned by groups that contain the uid
        let result = r"
            SELECT
                album_uuid
            FROM
                (
                SELECT
                    gid
                FROM
                    group_membership
                WHERE
                    uid = :uid
            ) AS t1
            INNER JOIN albums ON t1.gid = albums.gid
            WHERE
                CONCAT_WS(' ', name, note) LIKE :filter"
            .with(params! {
                "uid" => uid,
                "filter" => format!("%{}%", filter),
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let data = result
            .into_iter()
            .map(|row| from_row_opt::<AlbumUuid>(row))
            .collect::<Result<Vec<_>, FromRowError>>()?;

        Ok(data)
    }

    async fn search_media_in_album(
        &self,
        uid: String,
        album_uuid: AlbumUuid,
        filter: String,
    ) -> anyhow::Result<Vec<MediaUuid>> {
        // for a given uid, filter, and album_uuid, find all non-hidden media in that album
        // provided that the album is owned by a group containing the uid
        let result = r"
            SELECT
                media.media_uuid
            FROM
                (
                SELECT
                    media_uuid
                FROM
                    (
                    SELECT
                        album_uuid
                    FROM
                        (
                        SELECT
                            gid
                        FROM
                            group_membership
                        WHERE
                            uid = :uid
                    ) AS t1
                INNER JOIN albums ON t1.gid = albums.gid
                WHERE
                    album_uuid = :album_uuid
                ) AS t2
            INNER JOIN album_contents ON t2.album_uuid = album_contents.album_uuid
            ) AS t3
            INNER JOIN media ON t3.media_uuid = media.media_uuid
            WHERE
                hidden = FALSE AND CONCAT_WS(' ', date, note) LIKE :filter
            "
        .with(params! {
            "uid" => uid,
            "album_uuid" => album_uuid,
            "filter" => format!("%{}%", filter),
        })
        .run(self.pool.get_conn().await?)
        .await?
        .collect::<Row>()
        .await?;

        let data = result
            .into_iter()
            .map(|row| from_row_opt::<MediaUuid>(row))
            .collect::<Result<Vec<_>, FromRowError>>()?;

        Ok(data)
    }

    // library queries
    async fn add_library(&self, library: Library) -> anyhow::Result<LibraryUuid> {
        let mut result = r"
            INSERT INTO libraries (library_uuid, path, gid, file_count, last_scan)
            OUTPUT INSERTED.library_uuid
            VALUES (UUID_SHORT(), :path, :gid, :file_count, :last_scan)"
            .with(params! {
                "path" => library.path,
                "gid" => library.gid,
                "file_count" => library.metadata.file_count,
                "last_scan" => library.metadata.last_scan
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let row = result
            .pop()
            .ok_or_else(|| anyhow::Error::msg("Failed to return result for add_library"))?;

        let data = from_row_opt::<LibraryUuid>(row)?;

        Ok(data)
    }

    async fn get_library(&self, library_uuid: LibraryUuid) -> anyhow::Result<Option<Library>> {
        let mut result = r"
            SELECT path, gid, file_count, last_scan FROM libraries WHERE library_uuid = :library_uuid"
            .with(params! {
                "library_uuid" => library_uuid,
            }).run(self.pool.get_conn().await?)
            .await?.collect::<Row>()
            .await?;

        let row = match result.pop() {
            Some(row) => row,
            None => return Ok(None),
        };

        let data = from_row_opt::<(String, String, i64, i64)>(row)?;

        Ok(Some(Library {
            path: data.0,
            gid: data.1,
            metadata: LibraryMetadata {
                file_count: data.2,
                last_scan: data.3,
            },
        }))
    }

    async fn update_library(
        &self,
        library_uuid: LibraryUuid,
        change: LibraryMetadata,
    ) -> anyhow::Result<()> {
        r"
            UPDATE libraries SET file_count = :file_count, last_scan = :last_scan WHERE library_uuid = :library_uuid"
            .with(params! {
                "file_count" => change.file_count,
                "last_scan" => change.last_scan,
                "library_uuid" => library_uuid,
            }).run(self.pool.get_conn().await?)
            .await?;

        Ok(())
    }

    async fn search_libraries(
        &self,
        uid: String,
        filter: String,
    ) -> anyhow::Result<Vec<LibraryUuid>> {
        Err(anyhow::Error::msg("not implemented"))
    }

    async fn search_media_in_library(
        &self,
        uid: String,
        library_uuid: LibraryUuid,
        filter: String,
        hidden: bool,
    ) -> anyhow::Result<Vec<MediaUuid>> {
        // for a given uid, filter, hidden state and library_uuid, find all media in that album
        // provided that the library is owned by a group containing the uid
        //
        // note that this is the only search query where media with "hidden = true" can be found
        let result = r"
            SELECT
                media_uuid
            FROM
                (
                SELECT
                    library_uuid
                FROM
                    (
                    SELECT
                        gid
                    FROM
                        group_membership
                    WHERE
                        uid = :uid
                ) AS t1
            INNER JOIN libraries ON t1.gid = libraries.gid
            WHERE
                library_uuid = :library_uuid
            ) AS t2
            INNER JOIN media ON t2.library_uuid = media.library_uuid
            WHERE
                (
                    hidden = :hidden AND CONCAT_WS(' ', DATE, note) LIKE :filter
                )"
        .with(params! {
            "uid" => uid,
            "library_uuid" => library_uuid,
            "hidden" => hidden,
            "filter" => format!("%{}%", filter),
        })
        .run(self.pool.get_conn().await?)
        .await?
        .collect::<Row>()
        .await?;

        let data = result
            .into_iter()
            .map(|row| from_row_opt::<MediaUuid>(row))
            .collect::<Result<Vec<_>, FromRowError>>()?;

        Ok(data)
    }

    // ticket queries
    async fn create_ticket(&self, ticket: Ticket) -> anyhow::Result<TicketUuid> {
        let mut result = r"
            INSERT INTO tickets (ticket_uuid, media_uuid, uid, title, timestamp, resolved)
            OUTPUT INSERTED.ticket_uuid
            VALUES (UUID_SHORT(), :media_uuid, :uid, :title, :timestamp, :resolved)"
            .with(params! {
                "media_uuid" => ticket.media_uuid,
                "uid" => ticket.uid,
                "title" => ticket.title,
                "timestamp" => ticket.timestamp,
                "resolved" => ticket.resolved,
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let row = result
            .pop()
            .ok_or_else(|| anyhow::Error::msg("Failed to return result for create_ticket"))?;

        let data = from_row_opt::<TicketUuid>(row)?;

        Ok(data)
    }

    async fn create_comment(&self, comment: TicketComment) -> anyhow::Result<CommentUuid> {
        let mut result = r"
            INSERT INTO comments (comment_uuid, ticket_uuid, uid, text, timestamp)
            OUTPUT INSERTED.comment_uuid
            VALUES (UUID_SHORT(), :ticket_uuid, :uid, :text, :timestamp)"
            .with(params! {
                "ticket_uuid" => comment.ticket_uuid,
                "uid" => comment.uid,
                "text" => comment.text,
                "timestamp" => comment.timestamp,
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let row = result
            .pop()
            .ok_or_else(|| anyhow::Error::msg("Failed to return result for create_comment"))?;

        let data = from_row_opt::<CommentUuid>(row)?;

        Ok(data)
    }

    async fn get_ticket(&self, ticket_uuid: TicketUuid) -> anyhow::Result<Option<Ticket>> {
        let mut ticket_result = r"
            SELECT media_uuid, uid, title, timestamp, resolved FROM tickets WHERE ticket_uuid = :ticket_uuid"
            .with(params! {"ticket_uuid" => ticket_uuid}).run(self.pool.get_conn().await?)
            .await?.collect::<Row>()
            .await?;

        let ticket_row = match ticket_result.pop() {
            Some(row) => row,
            None => return Ok(None),
        };

        let ticket_data = from_row_opt::<(MediaUuid, String, String, i64, bool)>(ticket_row)?;

        let comment_result = r"
            SELECT comment_uuid, uid, text, timestamp FROM comments WHERE ticket_uuid = :ticket_uuid"
            .with(params! {"ticket_uuid" => ticket_uuid}).run(self.pool.get_conn().await?)
            .await?.collect::<Row>()
            .await?;

        let comment_rows = comment_result
            .into_iter()
            .map(|row| from_row_opt::<(CommentUuid, String, String, i64)>(row))
            .collect::<Result<Vec<_>, FromRowError>>()?;

        let mut comment_data = HashMap::new();

        for row in comment_rows.into_iter() {
            match comment_data.insert(
                row.0,
                TicketComment {
                    ticket_uuid: ticket_uuid,
                    uid: row.1,
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

        Ok(Some(Ticket {
            media_uuid: ticket_data.0,
            uid: ticket_data.1,
            title: ticket_data.2,
            timestamp: ticket_data.3,
            resolved: ticket_data.4,
            comments: comment_data,
        }))
    }

    async fn set_ticket_resolved(
        &self,
        ticket_uuid: TicketUuid,
        resolved: bool,
    ) -> anyhow::Result<()> {
        r"
            UPDATE tickets SET resolved = :resolved WHERE ticket_uuid = :ticket_uuid"
            .with(params! {
                "resolved" => resolved,
                "ticket_uuid" => ticket_uuid,
            })
            .run(self.pool.get_conn().await?)
            .await?;

        Ok(())
    }

    async fn search_tickets(
        &self,
        uid: String,
        filter: String,
        resolved: bool,
    ) -> anyhow::Result<Vec<TicketUuid>> {
        // for a given uid and filter, find all tickets associated with media that
        let result = r"
            SELECT
                ticket_uuid
            FROM
                (
                SELECT
                    media_uuid
                FROM
                    (
                    SELECT
                        album_uuid
                    FROM
                        (
                        SELECT
                            gid
                        FROM
                            group_membership
                        WHERE
                            uid = :uid
                    ) AS t1
                INNER JOIN albums ON t1.gid = albums.gid
                ) AS t2
            INNER JOIN album_contents ON t2.album_uuid = album_contents.album_uuid
            UNION
            SELECT
                media_uuid
            FROM
                (
                SELECT
                    library_uuid
                FROM
                    (
                    SELECT
                        gid
                    FROM
                        group_membership
                    WHERE
                        uid = :uid
                ) AS t1
            INNER JOIN libraries ON t1.gid = libraries.gid
            ) AS t2
            INNER JOIN media ON t2.library_uuid = media.library_uuid
            ) AS t3
            INNER JOIN tickets ON t3.media_uuid = tickets.media_uuid
            WHERE
                resolved = :resolved AND title LIKE :filter"
            .with(params! {
                "uid" => uid,
                "resolved" => resolved,
                "filter" => format!("%{}%", filter),
            })
            .run(self.pool.get_conn().await?)
            .await?
            .collect::<Row>()
            .await?;

        let data = result
            .into_iter()
            .map(|row| from_row_opt::<TicketUuid>(row))
            .collect::<Result<Vec<_>, FromRowError>>()?;

        Ok(data)
    }
}

#[async_trait]
impl ESInner for MySQLState {
    fn new(
        config: Arc<ESConfig>,
        senders: HashMap<ServiceType, ESMSender>,
    ) -> anyhow::Result<Self> {
        Ok(MySQLState {
            auth_svc_sender: senders.get(&ServiceType::Auth).unwrap().clone(),
            pool: Pool::new(config.mysql_url.clone().as_str()),
        })
    }

    async fn message_handler(&self, esm: ESM) -> anyhow::Result<()> {
        match esm {
            ESM::Db(message) => match message {
                // auth messages
                DbMsg::MediaAccessGroups { resp, media_uuid } => {
                    self.respond(resp, self.media_access_groups(media_uuid))
                        .await
                }
                DbMsg::CreateUser {
                    resp,
                    uid,
                    metadata,
                } => self.respond(resp, self.create_user(uid, metadata)).await,
                DbMsg::GetUser { resp, uid } => self.respond(resp, self.get_user(uid)).await,
                DbMsg::DeleteUser { resp, uid } => self.respond(resp, self.delete_user(uid)).await,
                DbMsg::CreateGroup {
                    resp,
                    gid,
                    metadata,
                } => self.respond(resp, self.create_group(gid, metadata)).await,
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

                // media messages
                DbMsg::AddMedia { resp, media } => self.respond(resp, self.add_media(media)).await,
                DbMsg::GetMedia { resp, media_uuid } => {
                    self.respond(resp, self.get_media(media_uuid)).await
                }
                DbMsg::GetMediaUuidByPath { resp, path } => {
                    self.respond(resp, self.get_media_uuid_by_path(path)).await
                }
                DbMsg::UpdateMedia {
                    resp,
                    media_uuid,
                    change,
                } => {
                    self.respond(resp, self.update_media(media_uuid, change))
                        .await
                }
                DbMsg::SetMediaHidden {
                    resp,
                    media_uuid,
                    hidden,
                } => {
                    self.respond(resp, self.set_media_hidden(media_uuid, hidden))
                        .await
                }
                DbMsg::SearchMedia { resp, uid, filter } => {
                    self.respond(resp, self.search_media(uid, filter)).await
                }

                // album messages
                DbMsg::CreateAlbum { resp, album } => {
                    self.respond(resp, self.create_album(album)).await
                }
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
                DbMsg::SearchAlbums { resp, uid, filter } => {
                    self.respond(resp, self.search_albums(uid, filter)).await
                }
                DbMsg::SearchMediaInAlbum {
                    resp,
                    uid,
                    album_uuid,
                    filter,
                } => {
                    self.respond(resp, self.search_media_in_album(uid, album_uuid, filter))
                        .await
                }

                // library messages
                DbMsg::AddLibrary { resp, library } => {
                    self.respond(resp, self.add_library(library)).await
                }
                DbMsg::GetLibrary { resp, library_uuid } => {
                    self.respond(resp, self.get_library(library_uuid)).await
                }
                DbMsg::UpdateLibrary {
                    resp,
                    library_uuid,
                    change,
                } => {
                    self.respond(resp, self.update_library(library_uuid, change))
                        .await
                }
                DbMsg::SearchLibraries { resp, uid, filter } => {
                    self.respond(resp, self.search_libraries(uid, filter))
                        .await
                }
                DbMsg::SearchMediaInLibrary {
                    resp,
                    uid,
                    library_uuid,
                    filter,
                    hidden,
                } => {
                    self.respond(
                        resp,
                        self.search_media_in_library(uid, library_uuid, filter, hidden),
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
                DbMsg::SetTicketResolved {
                    resp,
                    ticket_uuid,
                    resolved,
                } => {
                    self.respond(resp, self.set_ticket_resolved(ticket_uuid, resolved))
                        .await
                }
                DbMsg::SearchTickets {
                    resp,
                    uid,
                    filter,
                    resolved,
                } => {
                    self.respond(resp, self.search_tickets(uid, filter, resolved))
                        .await
                }
            },
            _ => Err(anyhow::Error::msg("not implemented")),
        }
    }
}

pub struct MySQLService {
    config: Arc<ESConfig>,
    receiver: Arc<Mutex<ESMReceiver>>,
    handle: AsyncCell<tokio::task::JoinHandle<anyhow::Result<()>>>,
}

#[async_trait]
impl EntanglementService for MySQLService {
    type Inner = MySQLState;

    fn create(config: Arc<ESConfig>) -> (ESMSender, Self) {
        let (tx, rx) = tokio::sync::mpsc::channel::<ESM>(32);

        (
            tx,
            MySQLService {
                config: config.clone(),
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
