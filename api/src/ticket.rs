use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::MediaUuid;

// structs and types

pub type TicketUuid = i64;
pub type CommentUuid = i64;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ticket {
    pub timestamp: i64,
    pub resolved: bool,
    pub user: String,
    pub media_uuid: MediaUuid,
    pub title: String,
    pub comments: HashMap<CommentUuid, TicketComment>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TicketComment {
    pub timestamp: i64,
    pub user: String,
    pub text: String,
}

// messages

// creates a ticket
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateTicketReq {
    pub media_uuid: MediaUuid,
    pub title: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateTicketResp {
    pub ticket_uuid: TicketUuid,
}

// create a new comment and associate it with a
// particular ticket
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateCommentReq {
    pub ticket_uuid: TicketUuid,
    pub comment_text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateCommentResp {
    pub comment_uuid: CommentUuid,
}

// fetch a specific ticket
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetTicketReq {
    pub ticket_uuid: TicketUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetTicketResp {
    pub ticket: Ticket,
}

// search tickets on their titles (and possibly comments)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TicketSearchReq {
    pub filter: String,
    pub resolved: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TicketSearchResp {
    pub tickets: Vec<TicketUuid>,
}
