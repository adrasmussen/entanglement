use serde::{Deserialize, Serialize};

use crate::media::MediaUuid;
use crate::message;

// structs and types

pub type TicketUuid = i64;
pub type CommentUuid = i64;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ticket {
    pub media_uuid: MediaUuid,
    pub uid: String,
    pub title: String,
    pub timestamp: i64,
    pub resolved: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TicketComment {
    pub ticket_uuid: TicketUuid,
    pub uid: String,
    pub text: String,
    pub timestamp: i64,
}

// messages

macro_rules! ticket_message {
    ($s:ident) => {
        message! {$s, "ticket"}
    };
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TicketMessage {
    CreateTicket(CreateTicketReq),
    CreateComment(CreateCommentReq),
    GetTicket(GetTicketReq),
    GetComment(GetCommentReq),
    SetTicketResolved(SetTicketResolvedReq),
    SearchTickets(SearchTicketsReq),
}

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

ticket_message! {CreateTicket}

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

ticket_message! {CreateComment}

// fetch a specific ticket
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetTicketReq {
    pub ticket_uuid: TicketUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetTicketResp {
    pub ticket: Ticket,
    pub comments: Vec<CommentUuid>,
}

ticket_message! {GetTicket}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetCommentReq {
    pub comment_uuid: CommentUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetCommentResp {
    pub comment: TicketComment,
}

ticket_message! {GetComment}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SetTicketResolvedReq {
    pub ticket_uuid: TicketUuid,
    pub resolved: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SetTicketResolvedResp {}

ticket_message! {SetTicketResolved}

// search tickets on their titles (and possibly comments)
//
// default is "" and false
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SearchTicketsReq {
    pub filter: String,
    pub resolved: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchTicketsResp {
    pub tickets: Vec<TicketUuid>,
}

ticket_message! {SearchTickets}
