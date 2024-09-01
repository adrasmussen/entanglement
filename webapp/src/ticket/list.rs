use dioxus::prelude::*;

use crate::common::{modal::Modal, style};
use api::ticket::*;

#[derive(Clone, PartialEq, Props)]
struct TicketListEntryProps {
    modal_stack_signal: Signal<Vec<Modal>>,
    ticket_uuid: TicketUuid,
}

#[component]
fn TicketListEntry(props: TicketListEntryProps) -> Element {
    let mut modal_stack_signal = props.modal_stack_signal;
    let ticket_uuid = props.ticket_uuid;

    let ticket = use_resource(move || async move {
        get_ticket(&GetTicketReq {
            ticket_uuid: ticket_uuid,
        })
        .await
    });

    let ticket = &*ticket.read();

    // this should throw a more informative error
    let result = match ticket {
        Some(Ok(result)) => result.ticket.clone(),
        _ => return rsx! {},
    };

    rsx! {
            tr {
                onclick: move |_| { modal_stack_signal.push(Modal::ShowTicket(ticket_uuid)) },
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
    modal_stack_signal: Signal<Vec<Modal>>,
    tickets: Vec<TicketUuid>,
}

#[component]
pub fn TicketList(props: TicketListProps) -> Element {
    rsx! {
        div {
            style { "{style::TABLE}" }
            table {
                tr {
                    th { "Media" }
                    th { "Creator" }
                    th { "Title" }
                    th { "Timestamp" }
                    th { "Resolved" }
                    th { "Comments" }
                }

                for ticket_uuid in props.tickets.iter() {
                    TicketListEntry { modal_stack_signal: props.modal_stack_signal, ticket_uuid: *ticket_uuid }
                }
            }
        }
    }
}
