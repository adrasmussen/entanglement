use std::collections::HashSet;

use chrono::Local;

use mysql_async::{from_row_opt, prelude::*, FromRowError, Pool, Row};

use api::album::{Album, AlbumUpdate, AlbumUuid};
use api::comment::{Comment, CommentUuid};
use api::library::{Library, LibraryUpdate, LibraryUuid};
use api::media::{Media, MediaMetadata, MediaUpdate, MediaUuid};
use crate::auth::{Group, User};

pub async fn media_access_groups(
    pool: Pool,
    media_uuid: MediaUuid,
) -> anyhow::Result<HashSet<String>> {
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
        .run(pool.get_conn().await?)
        .await?
        .collect::<Row>()
        .await?;

    let data = result
        .into_iter()
        .map(|row| from_row_opt::<String>(row))
        .collect::<Result<HashSet<_>, FromRowError>>()?;

    Ok(data)
}

pub async fn create_user(pool: Pool, uid: String, name: String) -> anyhow::Result<()> {
    r"
        INSERT INTO users (uid, name)
        VALUES (:uid, :name)"
        .with(params! {
            "uid" => uid.clone(),
            "name" => name,
        })
        .run(pool.get_conn().await?)
        .await?;

    r"
        INSERT INTO groups (gid)
        VALUES (:uid)"
        .with(params! {"uid" => uid.clone()})
        .run(pool.get_conn().await?)
        .await?;

    r"
        INSERT INTO group_membership (uid, gid)
        VALUES (:uid, :uid)"
        .with(params! {"uid" => uid.clone()})
        .run(pool.get_conn().await?)
        .await?;

    Ok(())
}

pub async fn get_user(pool: Pool, uid: String) -> anyhow::Result<Option<User>> {
    let mut user_result = r"
        SELECT uid, name FROM users WHERE uid = :uid"
        .with(params! {"uid" => uid.clone()})
        .run(pool.get_conn().await?)
        .await?
        .collect::<Row>()
        .await?;

    let user_row = match user_result.pop() {
        Some(row) => row,
        None => return Ok(None),
    };

    let user_data = from_row_opt::<(String, String)>(user_row)?;

    let group_result = r"
        SELECT gid FROM group_membership WHERE uid = :uid"
        .with(params! {"uid" => uid})
        .run(pool.get_conn().await?)
        .await?
        .collect::<Row>()
        .await?;

    let group_data = group_result
        .into_iter()
        .map(|row| from_row_opt::<String>(row))
        .collect::<Result<HashSet<_>, FromRowError>>()?;

    Ok(Some(User {
        uid: user_data.0,
        name: user_data.1,
        groups: group_data,
    }))
}

pub async fn delete_user(pool: Pool, uid: String) -> anyhow::Result<()> {
    r"
        DELETE FROM users WHERE uid = :uid"
        .with(params! {
            "uid" => uid.clone(),
        })
        .run(pool.get_conn().await?)
        .await?;

    r"
        DELETE FROM group_membership WHERE uid = :uid"
        .with(params! {
            "uid" => uid.clone(),
        })
        .run(pool.get_conn().await?)
        .await?;

    Ok(())
}

pub async fn create_group(pool: Pool, gid: String, name: String) -> anyhow::Result<()> {
    r"
        INSERT INTO groups (gid, name)
        VALUES (:gid, :name)"
        .with(params! {
            "gid" => gid,
            "name" => name,
        })
        .run(pool.get_conn().await?)
        .await?;

    Ok(())
}

pub async fn get_group(pool: Pool, gid: String) -> anyhow::Result<Option<Group>> {
    let mut group_rows = r"
        SELECT gid, name FROM groups WHERE gid = :gid"
        .with(params! {"gid" => gid.clone()})
        .run(pool.get_conn().await?)
        .await?
        .collect::<Row>()
        .await?;

    let group_row = match group_rows.pop() {
        Some(row) => row,
        None => return Ok(None),
    };

    let group_data = from_row_opt::<(String, String)>(group_row)?;

    let user_rows = r"
        SELECT uid FROM group_membership WHERE gid = :gid"
        .with(params! {"gid" => gid})
        .run(pool.get_conn().await?)
        .await?
        .collect::<Row>()
        .await?;

    let user_data = user_rows
        .into_iter()
        .map(|row| from_row_opt::<String>(row))
        .collect::<Result<HashSet<_>, FromRowError>>()?;

    Ok(Some(Group {
        gid: group_data.0,
        name: group_data.1,
        members: user_data,
    }))
}

pub async fn delete_group(pool: Pool, gid: String) -> anyhow::Result<()> {
    r"
        DELETE FROM users WHERE gid = :gid"
        .with(params! {
            "gid" => gid.clone(),
        })
        .run(pool.get_conn().await?)
        .await?;

    r"
        DELETE FROM group_membership WHERE gid = :gid"
        .with(params! {
            "gid" => gid,
        })
        .run(pool.get_conn().await?)
        .await?;

    Ok(())
}

pub async fn add_user_to_group(pool: Pool, uid: String, gid: String) -> anyhow::Result<()> {
    r"
        INSERT INTO group_membership (uid, gid)
        VALUES (:uid, :gid)"
        .with(params! {
            "uid" => uid.clone(),
            "gid" => gid,
        })
        .run(pool.get_conn().await?)
        .await?;

    Ok(())
}

pub async fn rm_user_from_group(pool: Pool, uid: String, gid: String) -> anyhow::Result<()> {
    r"
        DELETE FROM group_membership WHERE (uid = :uid AND gid = :gid)
        VALUES (:uid, :gid)"
        .with(params! {
            "uid" => uid.clone(),
            "gid" => gid,
        })
        .run(pool.get_conn().await?)
        .await?;

    Ok(())
}

pub async fn add_media(pool: Pool, media: Media) -> anyhow::Result<MediaUuid> {
    let mut result = r"
        INSERT INTO media (media_uuid, library_uuid, path, hash, mtime, hidden, attention, date, note, media_type)
        VALUES (UUID_SHORT(), :library_uuid, :path, :hash, :mtime, :hidden, :attention, :date, :note, :media_type)
        RETURNING media_uuid"
        .with(params! {
            "library_uuid" => media.library_uuid,
            "path" => media.path,
            "hash" => media.hash,
            "mtime" => Local::now().timestamp(),
            "hidden" => media.hidden,
            "attention" => media.attention,
            "date" => media.date,
            "note" => media.note,
            "media_type" => match media.metadata {
                MediaMetadata::Image => "Image",
                MediaMetadata::Video => "Video",
                MediaMetadata::VideoSlice => "VideoSlice",
                MediaMetadata::Audio => "Audio"
            },
        })
        .run(pool.get_conn().await?)
        .await?
        .collect::<Row>()
        .await?;

    let row = result
        .pop()
        .ok_or_else(|| anyhow::Error::msg("Failed to return result for add_media"))?;

    let data = from_row_opt::<MediaUuid>(row)?;

    // TODO -- add in the missing INSERT statements for the metadata tables

    Ok(data)
}

pub async fn get_media(
    pool: Pool,
    media_uuid: MediaUuid,
) -> anyhow::Result<Option<(Media, Vec<AlbumUuid>, Vec<CommentUuid>)>> {
    let mut media_result = r"
        SELECT library_uuid, path, hash, mtime, hidden, attention, date, note, media_type FROM media WHERE media_uuid = :media_uuid"
        .with(params! {
            "media_uuid" => media_uuid,
        })
        .run(pool.get_conn().await?)
        .await?
        .collect::<Row>()
        .await?;

    let media_data = match media_result.pop() {
        Some(row) => from_row_opt::<(
            LibraryUuid,
            String,
            String,
            i64,
            bool,
            bool,
            String,
            String,
            String,
        )>(row)?,
        None => return Ok(None),
    };

    let album_result = r"
        SELECT album_uuid FROM album_contents WHERE media_uuid = :media_uuid"
        .with(params! {
            "media_uuid" => media_uuid,
        })
        .run(pool.get_conn().await?)
        .await?
        .collect::<Row>()
        .await?;

    let album_data = album_result
        .into_iter()
        .map(|row| from_row_opt::<AlbumUuid>(row))
        .collect::<Result<Vec<_>, FromRowError>>()?;

    let comment_result = r"
        SELECT comment_uuid FROM comments WHERE media_uuid = :media_uuid"
        .with(params! {
            "media_uuid" => media_uuid,
        })
        .run(pool.get_conn().await?)
        .await?
        .collect::<Row>()
        .await?;

    let comment_data = comment_result
        .into_iter()
        .map(|row| from_row_opt::<CommentUuid>(row))
        .collect::<Result<Vec<_>, FromRowError>>()?;

    Ok(Some((
        Media {
            library_uuid: media_data.0,
            path: media_data.1,
            hash: media_data.2,
            mtime: media_data.3,
            hidden: media_data.4,
            attention: media_data.5,
            date: media_data.6,
            note: media_data.7,
            metadata: match media_data.8.as_str() {
                "Image" => MediaMetadata::Image,
                "Video" => MediaMetadata::Video,
                "VideoSlice" => MediaMetadata::VideoSlice,
                "Audio" => MediaMetadata::Audio,
                _ => {
                    return Err(anyhow::Error::msg(format!(
                        "invalid media record for {media_uuid}"
                    )))
                }
            },
        },
        album_data,
        comment_data,
    )))
}

pub async fn get_media_uuid_by_path(pool: Pool, path: String) -> anyhow::Result<Option<MediaUuid>> {
    let mut result = r"
        SELECT media_uuid FROM media WHERE path = :path"
        .with(params! {
            "path" => path,
        })
        .run(pool.get_conn().await?)
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

pub async fn update_media(
    pool: Pool,
    media_uuid: MediaUuid,
    update: MediaUpdate,
) -> anyhow::Result<()> {
    if let Some(val) = update.hidden {
        r"
        UPDATE media SET hidden = :hidden WHERE media_uuid = :media_uuid"
            .with(params! {
                "hidden" => val,
                "media_uuid" => media_uuid,
            })
            .run(pool.get_conn().await?)
            .await?;
    }

    if let Some(val) = update.attention {
        r"
        UPDATE media SET attention = :attention WHERE media_uuid = :media_uuid"
            .with(params! {
                "attention" => val,
                "media_uuid" => media_uuid,
            })
            .run(pool.get_conn().await?)
            .await?;
    }

    if let Some(val) = update.date {
        r"
        UPDATE media SET date = :date WHERE media_uuid = :media_uuid"
            .with(params! {
                "date" => val.clone(),
                "media_uuid" => media_uuid,
            })
            .run(pool.get_conn().await?)
            .await?;
    }

    if let Some(val) = update.note {
        r"
        UPDATE media SET note = :note WHERE media_uuid = :media_uuid"
            .with(params! {
                "note" => val.clone(),
                "media_uuid" => media_uuid,
            })
            .run(pool.get_conn().await?)
            .await?;
    }

    Ok(())
}

pub async fn search_media(
    pool: Pool,
    uid: String,
    filter: String,
) -> anyhow::Result<Vec<MediaUuid>> {
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
            hidden = FALSE AND CONCAT(' ', DATE, note) LIKE :filter;"
        .with(params! {
            "uid" => uid,
            "filter" => format!("%{}%", filter),
        })
        .run(pool.get_conn().await?)
        .await?
        .collect::<Row>()
        .await?;

    let data = result
        .into_iter()
        .map(|row| from_row_opt::<MediaUuid>(row))
        .collect::<Result<Vec<_>, FromRowError>>()?;

    Ok(data)
}

pub async fn add_album(pool: Pool, album: Album) -> anyhow::Result<AlbumUuid> {
    let mut result = r"
        INSERT INTO albums (album_uuid, uid, gid, mtime, name, note)
        VALUES (UUID_SHORT(), :uid, :gid, :mtime, :name, :note)
        RETURNING album_uuid"
        .with(params! {
            "uid" => album.uid,
            "gid" => album.gid,
            "mtime" => album.mtime,
            "name" => album.name,
            "note" => album.note,
        })
        .run(pool.get_conn().await?)
        .await?
        .collect::<Row>()
        .await?;

    let row = result
        .pop()
        .ok_or_else(|| anyhow::Error::msg("Failed to return result for create_album"))?;

    let data = from_row_opt::<AlbumUuid>(row)?;

    Ok(data)
}

pub async fn get_album(pool: Pool, album_uuid: AlbumUuid) -> anyhow::Result<Option<Album>> {
    let mut result = r"
        SELECT uid, gid, mtime, name, note FROM albums WHERE album_uuid = :album_uuid"
        .with(params! {
            "album_uuid" => album_uuid,
        })
        .run(pool.get_conn().await?)
        .await?
        .collect::<Row>()
        .await?;

    let row = match result.pop() {
        Some(row) => row,
        None => return Ok(None),
    };

    let data = from_row_opt::<(String, String, i64, String, String)>(row)?;

    Ok(Some(Album {
        uid: data.0,
        gid: data.1,
        mtime: data.2,
        name: data.3,
        note: data.4,
    }))
}

pub async fn delete_album(pool: Pool, album_uuid: AlbumUuid) -> anyhow::Result<()> {
    r"
        DELETE FROM album_contents WHERE album_uuid = :album_uuid;
        RETURNING media_uuid"
        .with(params! {
            "album_uuid" => album_uuid,
        })
        .run(pool.get_conn().await?)
        .await?;

    r"
        DELETE FROM albums WHERE album_uuid = :album_uuid;"
        .with(params! {
            "album_uuid" => album_uuid,
        })
        .run(pool.get_conn().await?)
        .await?;

    Ok(())
}

pub async fn update_album(
    pool: Pool,
    album_uuid: AlbumUuid,
    update: AlbumUpdate,
) -> anyhow::Result<()> {
    if let Some(val) = update.name {
        r"
        UPDATE albums SET name = :name WHERE album_uuid = :album_uuid"
            .with(params! {
                "name" => val.clone(),
                "album_uuid" => album_uuid,
            })
            .run(pool.get_conn().await?)
            .await?;
    }

    if let Some(val) = update.note {
        r"
        UPDATE albums SET note = :note WHERE album_uuid = :album_uuid"
            .with(params! {
                "note" => val.clone(),
                "album_uuid" => album_uuid,
            })
            .run(pool.get_conn().await?)
            .await?;
    }

    Ok(())
}

pub async fn add_media_to_album(
    pool: Pool,
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
        .run(pool.get_conn().await?)
        .await?;

    Ok(())
}

pub async fn rm_media_from_album(
    pool: Pool,
    media_uuid: MediaUuid,
    album_uuid: AlbumUuid,
) -> anyhow::Result<()> {
    r"
        DELETE FROM album_contents WHERE (media_uuid = :media_uuid AND album_uuid = :album_uuid)"
        .with(params! {
            "media_uuid" => media_uuid,
            "album_uuid" => album_uuid,
        })
        .run(pool.get_conn().await?)
        .await?;

    Ok(())
}

pub async fn search_albums(
    pool: Pool,
    uid: String,
    filter: String,
) -> anyhow::Result<Vec<AlbumUuid>> {
    // for a given uid and filter, find all albums owned by groups that contain that uid
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
        .run(pool.get_conn().await?)
        .await?
        .collect::<Row>()
        .await?;

    let data = result
        .into_iter()
        .map(|row| from_row_opt::<AlbumUuid>(row))
        .collect::<Result<Vec<_>, FromRowError>>()?;

    Ok(data)
}

pub async fn search_media_in_album(
    pool: Pool,
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
            hidden = FALSE AND CONCAT_WS(' ', DATE, note) LIKE :filter
        "
    .with(params! {
        "uid" => uid,
        "album_uuid" => album_uuid,
        "filter" => format!("%{}%", filter),
    })
    .run(pool.get_conn().await?)
    .await?
    .collect::<Row>()
    .await?;

    let data = result
        .into_iter()
        .map(|row| from_row_opt::<MediaUuid>(row))
        .collect::<Result<Vec<_>, FromRowError>>()?;

    Ok(data)
}

pub async fn add_library(pool: Pool, library: Library) -> anyhow::Result<LibraryUuid> {
    let mut result = r"
        INSERT INTO libraries (library_uuid, path, gid, mtime, count)
        VALUES (UUID_SHORT(), :path, :gid, :mtime, :count)
        RETURNING library_uuid"
        .with(params! {
            "path" => library.path,
            "gid" => library.gid,
            "mtime" => library.mtime,
            "count" => library.count,
        })
        .run(pool.get_conn().await?)
        .await?
        .collect::<Row>()
        .await?;

    let row = result
        .pop()
        .ok_or_else(|| anyhow::Error::msg("Failed to return result for add_library"))?;

    let data = from_row_opt::<LibraryUuid>(row)?;

    Ok(data)
}

pub async fn get_library(pool: Pool, library_uuid: LibraryUuid) -> anyhow::Result<Option<Library>> {
    let mut result = r"
        SELECT path, gid, mtime, count FROM libraries WHERE library_uuid = :library_uuid"
        .with(params! {
            "library_uuid" => library_uuid,
        })
        .run(pool.get_conn().await?)
        .await?
        .collect::<Row>()
        .await?;

    let row = match result.pop() {
        Some(row) => row,
        None => return Ok(None),
    };

    let data = from_row_opt::<(String, String, i64, i64)>(row)?;

    Ok(Some(Library {
        path: data.0,
        gid: data.1,
        mtime: data.2,
        count: data.3,
    }))
}

pub async fn update_library(
    pool: Pool,
    library_uuid: LibraryUuid,
    update: LibraryUpdate,
) -> anyhow::Result<()> {
    if let Some(val) = update.count {
        r"
        UPDATE libraries SET count = :count WHERE library_uuid = :library_uuid"
            .with(params! {
                "count" => val.clone(),
                "library_uuid" => library_uuid,
            })
            .run(pool.get_conn().await?)
            .await?;
    }

    Ok(())
}

pub async fn search_libraries(
    pool: Pool,
    uid: String,
    filter: String,
) -> anyhow::Result<Vec<LibraryUuid>> {
    let result = r"
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
        CONCAT_WS(' ', name, note) LIKE :filter"
        .with(params! {
            "uid" => uid,
            "filter" => format!("%{}%", filter),
        })
        .run(pool.get_conn().await?)
        .await?
        .collect::<Row>()
        .await?;

    let data = result
        .into_iter()
        .map(|row| from_row_opt::<LibraryUuid>(row))
        .collect::<Result<Vec<_>, FromRowError>>()?;

    Ok(data)
}

pub async fn search_media_in_library(
    pool: Pool,
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
    .run(pool.get_conn().await?)
    .await?
    .collect::<Row>()
    .await?;

    let data = result
        .into_iter()
        .map(|row| from_row_opt::<MediaUuid>(row))
        .collect::<Result<Vec<_>, FromRowError>>()?;

    Ok(data)
}

pub async fn add_comment(pool: Pool, comment: Comment) -> anyhow::Result<CommentUuid> {
    let mut result = r"
        INSERT INTO comments (comment_uuid, media_uuid, mtime, uid, text)
        VALUES (UUID_SHORT(), :media_uuid, :mtime, :uid, :text)
        RETURNING comment_uuid"
        .with(params! {
            "media_uuid" => comment.media_uuid,
            "mtime" => comment.mtime,
            "uid" => comment.uid,
            "text" => comment.text,
        })
        .run(pool.get_conn().await?)
        .await?
        .collect::<Row>()
        .await?;

    let row = result
        .pop()
        .ok_or_else(|| anyhow::Error::msg("Failed to return result for add_comment"))?;

    let data = from_row_opt::<CommentUuid>(row)?;

    Ok(data)
}

pub async fn get_comment(pool: Pool, comment_uuid: CommentUuid) -> anyhow::Result<Option<Comment>> {
    let mut result = r"
    SELECT media_uuid, mtime, uid, text FROM comments WHERE comment_uuid = :comment_uuid"
        .with(params! {
            "comment_uuid" => comment_uuid,
        })
        .run(pool.get_conn().await?)
        .await?
        .collect::<Row>()
        .await?;

    let row = match result.pop() {
        Some(row) => row,
        None => return Ok(None),
    };

    let data = from_row_opt::<(MediaUuid, i64, String, String)>(row)?;

    Ok(Some(Comment {
        media_uuid: data.0,
        mtime: data.1,
        uid: data.2,
        text: data.3,
    }))
}

pub async fn delete_comment(pool: Pool, comment_uuid: CommentUuid) -> anyhow::Result<()> {
    r"
        DELETE FROM comments WHERE (comment_uuid = :comment_uuid)"
        .with(params! {
            "comment_uuid" => comment_uuid,
        })
        .run(pool.get_conn().await?)
        .await?;

    Ok(())
}

pub async fn update_comment(
    pool: Pool,
    comment_uuid: CommentUuid,
    text: Option<String>,
) -> anyhow::Result<()> {
    if let Some(val) = text {
        r"
        UPDATE comments SET text = :text WHERE comment_uuid = :comment_uuid"
            .with(params! {
                "text" => val.clone(),
                "comment_uuid" => comment_uuid,
            })
            .run(pool.get_conn().await?)
            .await?;
    }

    Ok(())
}
