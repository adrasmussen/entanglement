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
