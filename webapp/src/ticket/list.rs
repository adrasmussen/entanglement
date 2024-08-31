use dioxus::prelude::*;

use crate::common::{modal::{Modal, ModalBox}, style};
use api::ticket::*;

#[derive(Clone, PartialEq, Props)]
pub struct TicketListEntryProps {
    pub modal_stack_signal: Signal<Vec<Modal>>,
    pub ticket_uuid: TicketUuid,
}

#[component]
pub fn TicketListEntry(props: TicketListEntryProps) -> Element {
    let mut modal_stack_signal = props.modal_stack_signal;
    let ticket_uuid = props.ticket_uuid;

    let ticket = use_resource(move || async move {
        get_ticket(&GetTicketReq {
            ticket_uuid: ticket_uuid,
        })
        .await
    });

    let ticket = &*ticket.read();

    let result = match ticket {
        Some(Ok(result)) => result.ticket.clone(),
        _ => return rsx! {}
    };

    rsx! {
            tr {
                onclick: move |_| { modal_stack_signal.push(Modal::Ticket(ticket_uuid)) },
                td { "{result.media_uuid}" }
                td { "{result.uid}" }
                td { "{result.title}" }
                td { "{result.timestamp}" }
                td { "{result.resolved}" }
                td { "{result.comments.len()}" }
            }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct TicketListProps {
    pub tickets: Result<Vec<TicketUuid>, String>
}

#[component]
pub fn TicketList(props: TicketListProps) -> Element {
    let modal_stack_signal = use_signal::<Vec<Modal>>(|| Vec::new());

    rsx! {
        ModalBox { stack_signal: modal_stack_signal }

        div {
            style { "{style::TABLE}" }
            match props.tickets {
                Ok(tickets) => rsx! {
                    table {
                        tr {
                            th { "Media" }
                            th { "Creator" }
                            th { "Title" }
                            th { "Timestamp" }
                            th { "Resolved" }
                            th { "Comments" }
                        }

                        for ticket_uuid in tickets.iter() {
                            TicketListEntry { modal_stack_signal: modal_stack_signal, ticket_uuid: *ticket_uuid }
                        }
                    }
                },
                Err(err) => rsx! {
                    span { "{err}" }
                }
            }
        }
    }
}
