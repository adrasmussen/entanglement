use crate::auth::AuthType;
use crate::service::ESMResp;

#[derive(Debug)]
pub enum AuthMsg {
    ClearGroupCache {
        resp: ESMResp<()>,
    },
    ClearAccessCache {
        resp: ESMResp<()>,
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
        gid: String,
    },
    CanAccessFile {
        resp: ESMResp<bool>,
        uid: String,
        file: String,
    },
}
