use std::collections::{HashMap, HashSet};

use api::{album::*, group::*, library::*, ticket::*, user::*, *};

use crate::service::*;

#[derive(Debug)]
pub enum DbMsg {
    AddUser {
        resp: ESMResp<()>,
        user: User,
    },
    GetUser {
        resp: ESMResp<User>,
        uid: String,
    },
    DeleteUser {
        resp: ESMResp<()>,
        uid: String,
    },
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
    AddMedia {
        resp: ESMResp<MediaUuid>,
        media: Media,
    },
    GetMedia {
        resp: ESMResp<Media>,
        user: String,
        uuid: MediaUuid,
    },
    UpdateMedia {
        resp: ESMResp<()>,
        user: String,
        uuid: MediaUuid,
        change: MediaMetadata,
    },
    SearchMedia {
        resp: ESMResp<HashMap<MediaUuid, Media>>,
        user: String,
        filter: String,
    },
    GetImageGroups {
        resp: ESMResp<HashSet<String>>,
        uuid: MediaUuid,
    },
    AddAlbum {
        resp: ESMResp<()>,
        user: String,
        album: Album,
    },
    GetAlbum {
        resp: ESMResp<Album>,
        user: String,
        uuid: AlbumUuid,
    },
    DeleteAlbum {
        resp: ESMResp<()>,
        user: String,
        uuid: AlbumUuid,
    },
    UpdateAlbum {
        resp: ESMResp<()>,
        user: String,
        uuid: AlbumUuid,
        change: AlbumMetadata,
    },
    SearchAlbums {
        resp: ESMResp<()>,
        user: String,
        filter: String,
    },
    SearchMediaInAlbum {
        resp: ESMResp<HashMap<MediaUuid, Media>>,
        user: String,
        uuid: AlbumUuid,
        filter: String,
    },
    AddLibrary {
        resp: ESMResp<()>,
        library: Library,
    },
    GetLibary {
        resp: ESMResp<Library>,
        uuid: LibraryUuid,
    },
    SearchMediaInLibrary {
        resp: ESMResp<HashMap<MediaUuid, Media>>,
        user: String,
        uuid: LibraryUuid,
        filter: String,
        hidden: bool,
    },
    CreateTicket {
        resp: ESMResp<TicketUuid>,
        ticket: Ticket,
    },
    CreateComment {
        resp: ESMResp<CommentUuid>,
        ticket_uuid: TicketUuid,
        comment: TicketComment,
    },
    GetTicket {
        resp: ESMResp<Ticket>,
        ticket_uuid: TicketUuid,
    },
    TicketSearch {
        resp: ESMResp<Vec<TicketUuid>>,
        filter: String,
        resolved: bool,
    },
}

impl From<DbMsg> for ESM {
    fn from(value: DbMsg) -> Self {
        ESM::Db(value)
    }
}
