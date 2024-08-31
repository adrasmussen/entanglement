use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::media::MediaUuid;

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
    pub comments: HashMap<CommentUuid, TicketComment>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TicketComment {
    pub ticket_uuid: TicketUuid,
    pub uid: String,
    pub text: String,
    pub timestamp: i64,
}

// messages

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TicketMessage {
    CreateTicket(CreateTicketReq),
    CreateComment(CreateCommentReq),
    GetTicket(GetTicketReq),
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

pub async fn get_ticket(req: &GetTicketReq) -> anyhow::Result<GetTicketResp> {
    let resp = gloo_net::http::Request::post("/entanglement/api/ticket")
        .json(&TicketMessage::GetTicket(req.clone()))?
        .send()
        .await?;

    if resp.ok() {
        Ok(resp.json().await?)
    } else {
        Err(anyhow::Error::msg(resp.text().await?))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SetTicketResolvedReq {
    pub ticket_uuid: TicketUuid,
    pub resolved: bool
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SetTicketResolvedResp {}

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

pub async fn search_tickets(req: &SearchTicketsReq) -> anyhow::Result<SearchTicketsResp> {
    let resp = gloo_net::http::Request::post("/entanglement/api/ticket")
        .json(&TicketMessage::SearchTickets(req.clone()))?
        .send()
        .await?;

    if resp.ok() {
        Ok(resp.json().await?)
    } else {
        Err(anyhow::Error::msg(resp.text().await?))
    }
}
