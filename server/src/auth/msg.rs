use std::collections::HashSet;

use api::{album::*, *};

use crate::auth::AuthType;
use crate::service::*;

#[derive(Debug)]
pub enum AuthMsg {
    ClearUserCache {
        resp: ESMResp<()>,
        uid: Option<String>,
    },
    ClearAccessCache {
        resp: ESMResp<()>,
        uuid: Option<MediaUuid>,
    },
    IsValidUser {
        resp: ESMResp<bool>,
        auth_type: AuthType,
        uid: String,
        password: String,
    },
    // this likely does not need to be a message -- almost nothing would ever construct it
    IsGroupMember {
        resp: ESMResp<bool>,
        uid: String,
        gid: HashSet<String>,
    },
    CanAccessMedia {
        resp: ESMResp<bool>,
        uid: String,
        media_uuid: MediaUuid,
    },
    OwnsMedia {
        resp: ESMResp<bool>,
        uid: String,
        media_uuid: MediaUuid,
    },
    // CanAccessAlbum {
    //     resp: ESMResp<bool>,
    //     uid: String,
    //     album_uuid: AlbumUuid,
    // },
    // OwnsAlbum {
    //     resp: ESMResp<bool>,
    //     uid: String,
    //     album_uuid: AlbumUuid,
    // },
    // CanAccessTicket
    // OwnsTicket
    // OwnsComment
}

impl From<AuthMsg> for ESM {
    fn from(msg: AuthMsg) -> Self {
        ESM::Auth(msg)
    }
}
