use std::collections::HashSet;

use api::{album::*, group::*, library::*, ticket::*, user::*, media::*};

use crate::service::*;

// TODO -- in basically all of these, we should switch to Option<> on the Get operations
// to signify not found (i.e. no rows)

#[derive(Debug)]
pub enum DbMsg {
    // auth messages
    MediaAccessGroups {
        resp: ESMResp<HashSet<String>>,
        media_uuid: MediaUuid,
    },

    // user messages
    CreateUser {
        resp: ESMResp<()>,
        user: User,
    },
    GetUser {
        resp: ESMResp<Option<User>>,
        uid: String,
    },

    // group messages
    CreateGroup {
        resp: ESMResp<()>,
        group: Group,
    },
    GetGroup {
        resp: ESMResp<Option<Group>>,
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
        resp: ESMResp<Option<Media>>,
        media_uuid: MediaUuid,
    },
    GetMediaUuidByPath {
        resp: ESMResp<Option<MediaUuid>>,
        path: String,
    },
    UpdateMedia {
        resp: ESMResp<()>,
        media_uuid: MediaUuid,
        change: MediaMetadata,
    },
    SetMediaHidden {
        resp: ESMResp<()>,
        media_uuid: MediaUuid,
        hidden: bool,
    },
    SearchMedia {
        resp: ESMResp<Vec<MediaUuid>>,
        uid: String,
        filter: String,
    },

    // album messages
    CreateAlbum {
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
        change: AlbumMetadata,
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
        user: String,
        filter: String,
    },
    SearchMediaInAlbum {
        resp: ESMResp<Vec<MediaUuid>>,
        user: String,
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
        change: LibraryMetadata,
    },
    SearchMediaInLibrary {
        resp: ESMResp<Vec<MediaUuid>>,
        uid: String,
        library_uuid: LibraryUuid,
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
        resp: ESMResp<Option<Ticket>>,
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
