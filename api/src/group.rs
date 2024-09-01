use std::collections::HashSet;

use serde::{Serialize, Deserialize};

use crate::message;

// structs and types

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Group {
    pub gid: String,
    pub metadata: GroupMetadata,
    pub members: HashSet<String>
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GroupMetadata {}

// messages

macro_rules! group_message {
    ($s:ident) => {
        message! {$s, "group"}
    };
}

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
    pub gid: String,
    pub metadata: GroupMetadata,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateGroupResp {}

group_message! {CreateGroup}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetGroupReq {
    pub gid: String
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetGroupResp {
    pub group: Group,
}

group_message! {GetGroup}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeleteGroupReq {
    pub gid: String
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeleteGroupResp {}

group_message! {DeleteGroup}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddUserToGroupReq {
    pub uid: String,
    pub gid: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddUserToGroupResp {}

group_message! {AddUserToGroup}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RmUserFromGroupReq {
    pub uid: String,
    pub gid: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RmUserFromGroupResp {}

group_message! {RmUserFromGroup}
