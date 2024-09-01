use dioxus::prelude::*;

use crate::common::style;
use api::{album::AlbumUuid, library::LibraryUuid, media::MediaUuid, ticket::TicketUuid};

mod media;
use media::MediaBox;

mod ticket;
use ticket::TicketBox;

pub enum Modal {
    ShowMedia(MediaUuid),
    ShowAlbum(AlbumUuid),
    CreateAlbum,
    ShowLibrary(LibraryUuid),
    AddLibrary,
    ShowTicket(TicketUuid),
    CreateTicket(MediaUuid),
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
                            Modal::ShowMedia(media_uuid) => rsx! { MediaBox { stack_signal: stack_signal, media_uuid: media_uuid } },
                            Modal::ShowAlbum(album_uuid) => rsx! { span { "{album_uuid}" } },
                            Modal::CreateAlbum => rsx! {},
                            Modal::ShowLibrary(library_uuid) => rsx! { span { "{library_uuid}" } },
                            Modal::AddLibrary => rsx! {},
                            Modal::ShowTicket(ticket_uuid) => rsx! { TicketBox { stack_signal: stack_signal, ticket_uuid: ticket_uuid }  },
                            Modal::CreateTicket(media_uuid) => rsx! {},
                        },
                        None => return rsx! {}
                    }
                }
            }
        }
    }
}
