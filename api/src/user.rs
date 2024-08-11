use std::collections::HashSet;

use serde::{Deserialize, Serialize};

// structs and types

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub uid: String,
    pub metadata: UserMetadata,
    pub groups: HashSet<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserMetadata {}

// messages

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum UserMessage {
    CreateUser(CreateUserReq),
}

// add a new user
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateUserReq {
    pub uid: String,
    pub metadata: UserMetadata,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateUserResp {}

// get user
