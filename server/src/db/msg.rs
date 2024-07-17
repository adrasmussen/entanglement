use std::collections::{HashMap, HashSet};

use api::auth::{Group, User};
use api::{image::*, MediaUuid};

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
    AddImage {
        resp: ESMResp<ImageUuid>,
        image: Image,
    },
    GetImage {
        resp: ESMResp<Image>,
        user: String,
        uuid: ImageUuid,
    },
    UpdateImage {
        resp: ESMResp<()>,
        user: String,
        uuid: ImageUuid,
        change: ImageMetadata,
    },
    SearchImages {
        resp: ESMResp<HashMap<ImageUuid, Image>>,
        user: String,
        filter: String,
    },
    GetImageGroups {
        resp: ESMResp<HashSet<String>>,
        uuid: ImageUuid,
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
    AddLibrary {
        resp: ESMResp<()>,
        library: Library,
    },
    GetLibary {
        resp: ESMResp<Library>,
        uuid: LibraryUuid,
    },
    UpdateLibrary {
        resp: ESMResp<()>,
        user: String,
        uuid: LibraryUuid,
        change: LibraryMetadata,
    },
}

impl From<DbMsg> for ESM {
    fn from(value: DbMsg) -> Self {
        ESM::Db(value)
    }
}
