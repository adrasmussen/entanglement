use std::collections::HashSet;

use common::api::media::MediaUuid;

use crate::auth::AuthType;
use crate::service::*;

#[derive(Debug)]
pub enum AuthMsg {
    ClearUserCache {
        resp: ESMResp<()>,
        uid: Vec<String>,
    },
    ClearAccessCache {
        resp: ESMResp<()>,
        uuid: Vec<MediaUuid>,
    },
    IsValidUser {
        resp: ESMResp<bool>,
        auth_type: AuthType,
        uid: String,
        password: String,
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
}

impl From<AuthMsg> for ESM {
    fn from(msg: AuthMsg) -> Self {
        ESM::Auth(msg)
    }
}
