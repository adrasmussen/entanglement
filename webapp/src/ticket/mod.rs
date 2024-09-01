use dioxus::prelude::*;

use crate::common::{storage::*, style};
use api::ticket::*;

mod list;
use list::TicketList;

impl SearchStorage for SearchTicketsReq {
    fn store(&self) -> () {
        set_local_storage("ticket_search_req", &self)
    }

    fn fetch() -> Self {
        match get_local_storage("ticket_search_req") {
            Ok(val) => val,
            Err(_) => Self::default(),
        }
    }
}

#[derive(Clone, PartialEq, Props)]
struct TicketNavBarProps {
    search_filter_signal: Signal<SearchTicketsReq>,
    search_status: String,
}

#[component]
fn TicketNavBar(props: TicketNavBarProps) -> Element {
    let mut search_filter_signal = props.search_filter_signal;

    rsx! {
        div {
            style { "{style::SUBNAV}" }
            div {
                class: "subnav",
                form {
                    onsubmit: move |event| {
                        let filter = match event.values().get("search_filter") {
                            Some(val) => val.as_value(),
                            None => String::from(""),
                        };

                        let resolved = match event.values().get("resolved") {
                            Some(val) => match val.as_value().as_str() {
                                "true" => true,
                                _ => false,
                            },
                            None => false,
                        };

                        let req = SearchTicketsReq{
                            filter: filter,
                            resolved: resolved,
                        };

                        search_filter_signal.set(req.clone());

                        req.store();
                    },
                    input {
                        name: "search_filter",
                        r#type: "text",
                        value: "{search_filter_signal().filter}",

                    },
                    label {
                        r#for: "resolved",
                        "Resolved"
                    },
                    input {
                        id: "resolved",
                        name: "resolved",
                        r#type: "checkbox",
                        checked: search_filter_signal().resolved,
                        value: "true",
                    },
                    input {
                        r#type: "submit",
                        value: "Search",
                    },
                },
                span { "Search History" },
                span { "{props.search_status}" },
            }
        }
    }
}

#[component]
pub fn Tickets() -> Element {
    let search_filter_signal = use_signal(|| SearchTicketsReq::fetch());

    let search_results =
        use_resource(move || async move { search_tickets(&search_filter_signal()).await });

    let search_results = &*search_results.read();

    let (results, status) = match search_results {
        Some(Ok(results)) => (
            Ok(results.tickets.clone()),
            format!("found {} results", results.tickets.len()),
        ),
        Some(Err(err)) => (Err(err.to_string()), format!("error while searching")),
        None => (
            Ok(Vec::new()),
            format!("still awaiting search_tickets future..."),
        ),
    };

    rsx! {
        TicketNavBar { search_filter_signal: search_filter_signal, search_status: status}
        TicketList { tickets: results }
    }
}
