use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::message;

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

macro_rules! user_message {
    ($s:ident) => {
        message! {$s, "user"}
    };
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum UserMessage {
    CreateUser(CreateUserReq),
    GetUser(GetUserReq),
    DeleteUser(DeleteUserReq),
}

// add a new user
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateUserReq {
    pub uid: String,
    pub metadata: UserMetadata,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateUserResp {}

user_message! {CreateUser}

// get user
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetUserReq {
    pub uid: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetUserResp {
    pub user: User,
}

user_message! {GetUser}

// delete user
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeleteUserReq {
    pub uid: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeleteUserResp {}

user_message! {DeleteUser}
