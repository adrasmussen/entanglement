use std::collections::HashSet;

use serde::{Serialize, Deserialize};

// structs and types

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub uid: String,
    pub groups: HashSet<String>,
    pub library: String,
    pub settings: UserSettings
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserSettings {
    pub theme: Option<String>,
}

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
