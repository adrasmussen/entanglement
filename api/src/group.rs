use std::collections::HashSet;

use serde::{Serialize, Deserialize};

// structs and types

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Group {
    pub gid: String,
    pub members: HashSet<String>
}

// messages

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GroupMessage {
    CreateGroup(CreateGroupReq),
    GetGroup(GetGroupReq),
    DeleteGroup(DeleteGroupReq),
    AddUserToGroup(AddUserToGroupReq),
    RmUserFromGroup(RmUserFromGroupReq),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateGroupReq {
    pub group: Group,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateGroupResp {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetGroupReq {
    pub gid: String
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetGroupResp {
    pub group: Group,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeleteGroupReq {
    pub gid: String
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeleteGroupResp {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddUserToGroupReq {
    pub uid: String,
    pub gid: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddUserToGroupResp {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RmUserFromGroupReq {
    pub uid: String,
    pub gid: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RmUserFromGroupResp {}
