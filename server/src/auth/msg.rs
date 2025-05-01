use std::collections::HashSet;

use api::media::MediaUuid;

use crate::service::*;

#[derive(Debug)]
pub enum AuthMsg {
    _ClearUserCache {
        resp: EsmResp<()>,
        uid: Vec<String>,
    },
    ClearAccessCache {
        resp: EsmResp<()>,
        media_uuid: Vec<MediaUuid>,
    },
    GroupsForUser {
        resp: EsmResp<HashSet<String>>,
        uid: String,
    },
    UsersInGroup {
        resp: EsmResp<HashSet<String>>,
        gid: String,
    },
    IsGroupMember {
        resp: EsmResp<bool>,
        uid: String,
        gid: HashSet<String>,
    },
    CanAccessMedia {
        resp: EsmResp<bool>,
        uid: String,
        media_uuid: MediaUuid,
    },
    OwnsMedia {
        resp: EsmResp<bool>,
        uid: String,
        media_uuid: MediaUuid,
    },
    _AuthenticateUser {
        resp: EsmResp<bool>,
        uid: String,
        password: String,
    },
    _IsValidUser {
        resp: EsmResp<bool>,
        uid: String,
    },
}

impl From<AuthMsg> for Esm {
    fn from(msg: AuthMsg) -> Self {
        Esm::Auth(msg)
    }
}
