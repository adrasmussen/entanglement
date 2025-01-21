use dioxus::prelude::*;

use serde::{Deserialize, Serialize};

use crate::common::{
    modal::{Modal, ModalBox},
    storage::*,
    style,
};
use api::ticket::*;

mod list;
use list::TicketList;

const TICKET_SEARCH_KEY: &str = "ticket_search";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
struct StoredTicketSearch {
    filter: String,
    resolved: bool,
}

#[derive(Clone, PartialEq, Props)]
struct TicketNavBarProps {
    ticket_search_signal: Signal<StoredTicketSearch>,
    status: String,
}

#[component]
fn TicketNavBar(props: TicketNavBarProps) -> Element {
    let mut ticket_search_signal = props.ticket_search_signal;
    let status = props.status;

    rsx! {
        div {
            style { "{style::SUBNAV}" }
            div { class: "subnav",
                form {
                    onsubmit: move |event| {
                        let filter = match event.values().get("search_filter") {
                            Some(val) => val.as_value(),
                            None => String::from(""),
                        };
                        let resolved = match event.values().get("resolved") {
                            Some(val) => {
                                match val.as_value().as_str() {
                                    "true" => true,
                                    _ => false,
                                }
                            }
                            None => false,
                        };
                        let search = StoredTicketSearch {
                            filter: filter,
                            resolved: resolved,
                        };
                        ticket_search_signal.set(search.clone());
                        set_local_storage(TICKET_SEARCH_KEY, search)
                    },
                    input {
                        name: "search_filter",
                        r#type: "text",
                        value: "{ticket_search_signal().filter}"
                    }
                    label { r#for: "resolved", "Resolved" }
                    input {
                        id: "resolved",
                        name: "resolved",
                        r#type: "checkbox",
                        checked: ticket_search_signal().resolved,
                        value: "true"
                    }
                    input { r#type: "submit", value: "Search" }
                }
                span { "Search History" }
                span { "{status}" }
            }
        }
    }
}

#[component]
pub fn Tickets() -> Element {
    let modal_stack_signal = use_signal::<Vec<Modal>>(|| Vec::new());
    let ticket_search_signal =
        use_signal::<StoredTicketSearch>(|| try_local_storage(TICKET_SEARCH_KEY));

    let ticket_future = use_resource(move || async move {
        let search = ticket_search_signal();

        search_tickets(&SearchTicketsReq {
            filter: search.filter,
            resolved: search.resolved,
        })
        .await
    });

    let (tickets, status) = match &*ticket_future.read() {
        Some(Ok(resp)) => (
            Ok(resp.tickets.clone()),
            format!("Found {} results", resp.tickets.len()),
        ),
        Some(Err(err)) => (
            Err(err.to_string()),
            String::from("Error from search_tickets"),
        ),
        None => (
            Err(String::from("Still waiting on search_tickets future...")),
            String::from(""),
        ),
    };

    rsx! {
        TicketNavBar { ticket_search_signal, status }
        ModalBox { stack_signal: modal_stack_signal }

        match tickets {
            Ok(tickets) => rsx! {
                TicketList { modal_stack_signal: modal_stack_signal, tickets: tickets }
            },
            Err(err) => rsx! {
                span { "{err}" }
            },
        }
    }
}
