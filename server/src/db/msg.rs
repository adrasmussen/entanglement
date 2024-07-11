use std::collections::HashMap;

use api::image::*;
use api::auth::{User, Group};

use crate::service::ESMResp;

#[derive(Debug)]
pub enum DbMsg {
    AddUser,
    GetUser,
    DeleteUser,
    AddGroup,
    GetGroup {
        resp: ESMResp<Group>, // this should fail if the group does not exist
        gid: String,
    },
    DeleteGroup,
    AddUserToGroup,
    RmUserFromGroup,
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
    AddAlbum {
        resp: ESMResp<()>,
        user: String,
        uuid: AlbumUuid,
    },
    GetAlbum {
        resp: ESMResp<AlbumUuid>,
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
