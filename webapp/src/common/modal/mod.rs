use dioxus::prelude::*;

use crate::common::style;
use api::{album::AlbumUuid, library::LibraryUuid, media::MediaUuid, ticket::TicketUuid};

mod media;
use media::MediaBox;

mod ticket;
use ticket::TicketBox;

pub enum Modal {
    Media(MediaUuid),
    Album(AlbumUuid),
    Ticket(TicketUuid),
    Library(LibraryUuid),
}

#[derive(Clone, PartialEq, Props)]
pub struct ModalBoxProps {
    stack_signal: Signal<Vec<Modal>>,
}

#[component]
pub fn ModalBox(props: ModalBoxProps) -> Element {
    let mut stack_signal = props.stack_signal;

    rsx! {
        div {
            style { "{style::MODAL}" }
            div {
                class: "modal",
                div {
                    class: "modal-content",
                    div {
                        class: "modal-header",
                        span {
                            class: "close",
                            onclick: move |_| {stack_signal.pop();},
                            "X"
                        }
                    }
                    match stack_signal.last() {
                        Some(val) => match *val {
                            Modal::Media(media_uuid) => rsx! { MediaBox { stack_signal: stack_signal, media_uuid: media_uuid } },
                            Modal::Album(album_uuid) => rsx! { span { "{album_uuid}" } },
                            Modal::Ticket(ticket_uuid) => rsx! { TicketBox { stack_signal: stack_signal, ticket_uuid: ticket_uuid }  },
                            Modal::Library(library_uuid) => rsx! { span { "{library_uuid}" } },
                        },
                        None => return rsx! {}
                    }
                }
            }
        }
    }
}
