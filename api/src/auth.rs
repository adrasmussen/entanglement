use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::http_endpoint;

// structs and types

// messages

// look up users in a group
http_endpoint!(GetUsersInGroup);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GetUsersInGroupReq {
    pub gid: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GetUsersInGroupResp {
    pub uids: HashSet<String>,
}

// look up the current user and the groups they belong to
http_endpoint!(GetCurrentUser);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GetCurrentUserReq {}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GetCurrentUserResp {
    pub uid: String,
    // sorted, so that the order is deterministic (the server keeps them in a set)
    pub groups: Vec<String>,
}
