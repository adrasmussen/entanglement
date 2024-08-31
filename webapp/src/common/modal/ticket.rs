use dioxus::prelude::*;

use crate::common::{modal::Modal, stream::*};
use api::ticket::*;

#[derive(Clone, PartialEq, Props)]
pub struct TicketBoxProps {
    stack_signal: Signal<Vec<Modal>>,
    ticket_uuid: TicketUuid,
}

#[component]
pub fn TicketBox(props: TicketBoxProps) -> Element {
    let mut stack_signal = props.stack_signal;
    let ticket_uuid = props.ticket_uuid;

    let ticket = use_resource(move || async move {
        get_ticket(&GetTicketReq {
            ticket_uuid: ticket_uuid,
        })
        .await
    });

    let ticket = &*ticket.read();

    let result = match ticket {
        Some(result) => result,
        None => {
            return rsx! {
                div {
                    span { "unknown ticket_uuid {ticket_uuid}" }
                }
            }
        }
    };

    rsx! {
        div {
            class: "modal-body",
            match result {
                Ok(result) => {
                    let media_uuid = result.ticket.media_uuid.clone();

                    rsx! {
                        div {
                            img {
                                onclick: move |_| { stack_signal.push(Modal::Media(media_uuid)) },

                                src: full_link(media_uuid),
                            }
                        }
                        div {
                            form {
                                class: "modal-info",

                                label { "Media" },
                                span { "{result.ticket.media_uuid}" },

                                label { "Creator" },
                                span { "{result.ticket.uid}" },

                                label { "title" },
                                span { "{result.ticket.title}" },

                                label { "Timestamp" },
                                span { "{result.ticket.timestamp}"}

                                label { "Resolved" },
                                span { "{result.ticket.timestamp}"}
                            },
                        }
                        div {
                            float: "bottom",
                            span { "Comments" }

                            div {
                                class: "modal-info",

                                for (_, comment) in result.ticket.comments.iter() {
                                    label { "{comment.uid} ({comment.timestamp})"}
                                    span { "{comment.text}" }
                                }
                            }
                        }
                    }
                },
                Err(err) => rsx! {
                    span { "{err.to_string()}" }
                }
            }
        }
    }
}
