use crate::MediaUuid;

pub struct Ticket {
    pub user: String,
    pub uuid: MediaUuid,
    pub title: String,
    pub comments: Vec<TicketComment>,
}

pub struct TicketComment {
    pub user: String,
    pub date: String,
    pub text: String,
}

// need to impl PartialOrd on the date field
