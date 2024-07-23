use std::collections::HashSet;

use serde::{Serialize, Deserialize};

// structs and types

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Group {
    pub gid: String,
    pub members: HashSet<String>
}

// messages

// add a new user
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddUserReq {
    user: User,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddUserResp {
    resp: String,
}

// get user

// add group

// delete group

// get group

// add user to group

// remove user from group
