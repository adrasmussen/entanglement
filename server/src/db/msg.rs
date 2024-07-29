use std::collections::{HashMap, HashSet};

use api::{album::*, group::*, library::*, ticket::*, user::*, *};

use crate::service::*;

#[derive(Debug)]
pub enum DbMsg {
    // user messages
    AddUser {
        resp: ESMResp<()>,
        user: User,
    },
    GetUser {
        resp: ESMResp<User>,
        uid: String,
    },

    // group messages
    AddGroup {
        resp: ESMResp<()>,
        group: Group,
    },
    GetGroup {
        resp: ESMResp<Group>,
        gid: String,
    },
    DeleteGroup {
        resp: ESMResp<()>,
        gid: String,
    },
    AddUserToGroup {
        resp: ESMResp<()>,
        uid: String,
        gid: String,
    },
    RmUserFromGroup {
        resp: ESMResp<()>,
        uid: String,
        gid: String,
    },

    // media messages
    AddMedia {
        resp: ESMResp<MediaUuid>,
        media: Media,
    },
    GetMedia {
        resp: ESMResp<Media>,
        uuid: MediaUuid,
    },
    UpdateMedia {
        resp: ESMResp<()>,
        uuid: MediaUuid,
        change: MediaMetadata,
    },
    SearchMedia {
        resp: ESMResp<Vec<MediaUuid>>,
        user: String,
        filter: String,
    },
    CanAccessMedia {
        resp: ESMResp<bool>,
        user: String,
        media_uuid: MediaUuid,
    },

    // album messages
    AddAlbum {
        resp: ESMResp<()>,
        album: Album,
    },
    GetAlbum {
        resp: ESMResp<Album>,
        uuid: AlbumUuid,
    },
    DeleteAlbum {
        resp: ESMResp<()>,
        uuid: AlbumUuid,
    },
    UpdateAlbum {
        resp: ESMResp<()>,
        uuid: AlbumUuid,
        change: AlbumMetadata,
    },
    SearchAlbums {
        resp: ESMResp<Vec<AlbumUuid>>,
        user: String,
        filter: String,
    },
    SearchMediaInAlbum {
        resp: ESMResp<Vec<MediaUuid>>,
        user: String,
        uuid: AlbumUuid,
        filter: String,
    },

    // library messages
    AddLibrary {
        resp: ESMResp<LibraryUuid>,
        library: Library,
    },
    GetLibary {
        resp: ESMResp<Library>,
        uuid: LibraryUuid,
    },
    SearchMediaInLibrary {
        resp: ESMResp<Vec<MediaUuid>>,
        user: String,
        uuid: LibraryUuid,
        filter: String,
        hidden: bool,
    },

    // ticket messages
    CreateTicket {
        resp: ESMResp<TicketUuid>,
        ticket: Ticket,
    },
    CreateComment {
        resp: ESMResp<CommentUuid>,
        comment: TicketComment,
    },
    GetTicket {
        resp: ESMResp<Ticket>,
        ticket_uuid: TicketUuid,
    },
    SearchTickets {
        resp: ESMResp<Vec<TicketUuid>>,
        user: String,
        filter: String,
        resolved: bool,
    },
}

impl From<DbMsg> for ESM {
    fn from(value: DbMsg) -> Self {
        ESM::Db(value)
    }
}
