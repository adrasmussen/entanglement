use dioxus::prelude::*;

use crate::common::style;
use api::ticket::*;

#[derive(Clone, PartialEq, Props)]
pub struct TicketListEntryProps {
    pub view_ticket_signal: Signal<Option<TicketUuid>>,
    pub ticket_uuid: TicketUuid,
}

#[component]
pub fn TicketListEntry(props: TicketListEntryProps) -> Element {
    let mut view_ticket_signal = props.view_ticket_signal;
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
                td { "{result.media_uuid}" }
                td { "{result.uid}" }
                td { "{result.title}" }
                td { "{result.timestamp}" }
                td { "{result.resolved}" }
            }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct TicketListProps {
    pub tickets: Result<Vec<TicketUuid>, String>
}

#[component]
pub fn TicketList(props: TicketListProps) -> Element {
    let view_ticket_signal = use_signal::<Option<TicketUuid>>(|| None);

    rsx! {
        // TicketBox

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
                        }

                        for ticket_uuid in tickets.iter() {
                            TicketListEntry { view_ticket_signal: view_ticket_signal, ticket_uuid: *ticket_uuid }
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
