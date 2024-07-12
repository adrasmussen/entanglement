use std::collections::HashSet;

use api::MediaUuid;

use crate::auth::AuthType;
use crate::service::ESMResp;

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
    IsGroupMember {
        resp: ESMResp<bool>,
        uid: String,
        gid: HashSet<String>,
    },
    CanAccessMedia {
        resp: ESMResp<bool>,
        uid: String,
        uuid: MediaUuid,
    },
}
