use std::collections::HashSet;

use api::{album::*, comment::*, library::*, media::*, task::*};

use crate::service::*;

#[derive(Debug)]
pub enum DbMsg {
    // auth messages
    MediaAccessGroups {
        resp: ESMResp<HashSet<String>>,
        media_uuid: MediaUuid,
    },

    // media messages
    AddMedia {
        resp: ESMResp<MediaUuid>,
        media: Media,
    },
    GetMedia {
        resp: ESMResp<Option<(Media, Vec<AlbumUuid>, Vec<CommentUuid>)>>,
        media_uuid: MediaUuid,
    },
    GetMediaUuidByPath {
        resp: ESMResp<Option<MediaUuid>>,
        path: String,
    },
    UpdateMedia {
        resp: ESMResp<()>,
        media_uuid: MediaUuid,
        update: MediaUpdate,
    },
    SearchMedia {
        resp: ESMResp<Vec<MediaUuid>>,
        uid: String,
        gid: HashSet<String>,
        filter: String,
    },
    SimilarMedia {
        resp: ESMResp<Vec<MediaUuid>>,
        uid: String,
        gid: HashSet<String>,
        media_uuid: MediaUuid,
        distance: i64,
    },

    // comment messages
    AddComment {
        resp: ESMResp<CommentUuid>,
        comment: Comment,
    },
    GetComment {
        resp: ESMResp<Option<Comment>>,
        comment_uuid: CommentUuid,
    },
    DeleteComment {
        resp: ESMResp<()>,
        comment_uuid: CommentUuid,
    },
    UpdateComment {
        resp: ESMResp<()>,
        comment_uuid: CommentUuid,
        text: Option<String>,
    },

    // album messages
    AddAlbum {
        resp: ESMResp<AlbumUuid>,
        album: Album,
    },
    GetAlbum {
        resp: ESMResp<Option<Album>>,
        album_uuid: AlbumUuid,
    },
    DeleteAlbum {
        resp: ESMResp<()>,
        album_uuid: AlbumUuid,
    },
    UpdateAlbum {
        resp: ESMResp<()>,
        album_uuid: AlbumUuid,
        update: AlbumUpdate,
    },
    AddMediaToAlbum {
        resp: ESMResp<()>,
        media_uuid: MediaUuid,
        album_uuid: AlbumUuid,
    },
    RmMediaFromAlbum {
        resp: ESMResp<()>,
        media_uuid: MediaUuid,
        album_uuid: AlbumUuid,
    },
    SearchAlbums {
        resp: ESMResp<Vec<AlbumUuid>>,
        uid: String,
        gid: HashSet<String>,
        filter: String,
    },
    SearchMediaInAlbum {
        resp: ESMResp<Vec<MediaUuid>>,
        uid: String,
        gid: HashSet<String>,
        album_uuid: AlbumUuid,
        filter: String,
    },

    // library messages
    AddLibrary {
        resp: ESMResp<LibraryUuid>,
        library: Library,
    },
    GetLibrary {
        resp: ESMResp<Option<Library>>,
        library_uuid: LibraryUuid,
    },
    UpdateLibrary {
        resp: ESMResp<()>,
        library_uuid: LibraryUuid,
        update: LibraryUpdate,
    },
    SearchLibraries {
        resp: ESMResp<Vec<LibraryUuid>>,
        uid: String,
        gid: HashSet<String>,
        filter: String,
    },
    SearchMediaInLibrary {
        resp: ESMResp<Vec<MediaUuid>>,
        uid: String,
        gid: HashSet<String>,
        library_uuid: LibraryUuid,
        filter: String,
        hidden: bool,
    },

    // task messages
    AddTask {
        resp: ESMResp<TaskUuid>,
        task: Task,
    },
    GetTask {
        resp: ESMResp<Task>,
        task_uuid: TaskUuid,
    },
    DeleteTask {
        resp: ESMResp<()>,
        task_uuid: TaskUuid
    },
    UpdateTask {
        resp: ESMResp<()>,
        task_uuid: TaskUuid,
        update: TaskUpdate,
    },
    SearchTasks {
        resp: ESMResp<Vec<TaskUuid>>,
        filter: Option<TaskStatus>,
    },
    AddLog {
        resp: ESMResp<()>,
        log: String
    },
    GetLog {
        resp: ESMResp<String>,
        log_uuid: String,
    },
    DeleteLog {
        resp: ESMResp<()>,
        log_uuid: String,
    },
    SearchLogs {
        resp: ESMResp<Vec<String>>,
        filter: String,
    }
}

impl From<DbMsg> for ESM {
    fn from(value: DbMsg) -> Self {
        ESM::Db(value)
    }
}
