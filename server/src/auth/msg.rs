use std::collections::HashSet;

use api::media::MediaUuid;

use crate::service::*;

#[derive(Debug)]
pub enum AuthMsg {
    ClearUserCache {
        resp: ESMResp<()>,
        uid: Vec<String>,
    },
    ClearAccessCache {
        resp: ESMResp<()>,
        media_uuid: Vec<MediaUuid>,
    },
    GroupsForUser {
        resp: ESMResp<HashSet<String>>,
        uid: String,
    },
    UsersInGroup {
        resp: ESMResp<HashSet<String>>,
        gid: String,
    },
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
    AuthenticateUser {
        resp: ESMResp<bool>,
        uid: String,
        password: String,
    },
    IsValidUser {
        resp: ESMResp<bool>,
        uid: String,
    },
}

impl From<AuthMsg> for ESM {
    fn from(msg: AuthMsg) -> Self {
        ESM::Auth(msg)
    }
}
