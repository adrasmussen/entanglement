use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::endpoint;

// structs and types

// messages

// look up users in a group
endpoint!(GetUsersInGroup);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetUsersInGroupReq {
    pub gid: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetUsersInGroupResp {
    pub uids: HashSet<String>,
}
