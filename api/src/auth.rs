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
