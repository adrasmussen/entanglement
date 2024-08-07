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

// messages

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum UserMessage {
    CreateUser(CreateUserReq),
}

// add a new user
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateUserReq {
    pub user: User,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateUserResp {}

// get user
