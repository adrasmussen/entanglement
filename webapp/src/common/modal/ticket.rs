use dioxus::prelude::*;

use crate::common::{
    modal::{modal_err, Modal},
    stream::*,
};
use api::ticket::*;

#[derive(Clone, PartialEq, Props)]
pub struct ShowTicketBoxProps {
    stack_signal: Signal<Vec<Modal>>,
    ticket_uuid: TicketUuid,
}

#[component]
pub fn ShowTicketBox(props: ShowTicketBoxProps) -> Element {
    let mut stack_signal = props.stack_signal;
    let ticket_uuid = props.ticket_uuid;

    let ticket_future = use_resource(move || async move {
        get_ticket(&GetTicketReq {
            ticket_uuid: ticket_uuid,
        })
        .await
    });

    let (ticket, comments) = match &*ticket_future.read() {
        Some(Ok(resp)) => (resp.ticket.clone(), resp.comments.clone()),
        Some(Err(err)) => return modal_err(err.to_string()),
        None => return modal_err("Still waiting on get_ticket future..."),
    };

    let media_uuid = ticket.media_uuid.clone();

    rsx! {
        div { class: "modal-body",
            div {
                img {
                    onclick: move |_| { stack_signal.push(Modal::ShowMedia(media_uuid)) },

                    src: full_link(media_uuid)
                }
            }
            div {
                form { class: "modal-info",

                    label { "Media" }
                    span { "{ticket.media_uuid}" }

                    label { "Creator" }
                    span { "{ticket.uid}" }

                    label { "title" }
                    span { "{ticket.title}" }

                    label { "Timestamp" }
                    span { "{ticket.timestamp}" }

                    label { "Resolved" }
                    span { "{ticket.resolved}" }
                }
            }
            div { float: "bottom",
                span { "Comments" }

                div { class: "modal-info",

                    for comment_uuid in comments.iter() {
                        span { "{comment_uuid}" }
                    }
                }
            }
        }
    }
}
