use dioxus::prelude::*;

use crate::common::{
    modal::{modal_err, Modal},
    stream::*,
};
use api::media::*;

#[derive(Clone, PartialEq, Props)]
pub struct ShowMediaBoxProps {
    stack_signal: Signal<Vec<Modal>>,
    media_uuid: MediaUuid,
}

#[component]
pub fn ShowMediaBox(props: ShowMediaBoxProps) -> Element {
    let stack_signal = props.stack_signal;
    let media_uuid = props.media_uuid;

    let status_signal = use_signal(|| String::from(""));

    let media_future = use_resource(move || async move {
        status_signal();

        get_media(&GetMediaReq {
            media_uuid: media_uuid,
        })
        .await
    });

    let (media, albums, tickets) = match &*media_future.read() {
        Some(Ok(resp)) => (resp.media.clone(), resp.albums.clone(), resp.tickets.clone()),
        Some(Err(err)) => return modal_err(err.to_string()),
        None => return modal_err("Still waiting on get_media future..."),
    };

    rsx! {
        div {
            class: "modal-body",
            div {
                img {
                    src: full_link(media_uuid),
                }
            }
            div {
                form {
                    class: "modal-info",
                    onsubmit: move |event| async move {
                        let mut status_signal = status_signal;

                        let date = match event.values().get("date") {
                            Some(val) => val.as_value(),
                            None => String::from(""),
                        };

                        let note = match event.values().get("note") {
                            Some(val) => val.as_value(),
                            None => String::from(""),
                        };

                        let result = match update_media(&UpdateMediaReq {
                            media_uuid: media_uuid.clone(),
                            change: MediaMetadata {
                                date: date,
                                note: note,
                            }
                        }).await {
                            Ok(_) => String::from("Metadata updated successfully"),
                            Err(err) => format!("Error updating metadata: {}", err.to_string()),
                        };

                        status_signal.set(result)
                    },

                    label { "Library" },
                    span { "{media.library_uuid}" },

                    label { "Path" },
                    span { "{media.path}" },

                    label { "Hidden" },
                    span { "{media.hidden}" },

                    label { "Date" },
                    input {
                        name: "date",
                        r#type: "text",
                        value: "{media.metadata.date}"
                    },

                    label { "Note" },
                    textarea {
                        name: "note",
                        rows: "8",
                        value: "{media.metadata.note}"
                    },

                    input {
                        r#type: "submit",
                        value: "Update metadata",
                    }

                    div {
                        grid_column: "2",

                        button {
                            onclick: move |_| {
                                let mut stack_signal = stack_signal;

                                stack_signal.push(Modal::CreateTicket(media_uuid))
                            },
                            r#type: "button",
                            "Create ticket"
                        }
                        button {
                            onclick: move |_| {},
                            r#type: "button",
                            "Albums"
                        }
                        button {
                            onclick: move |_| async move {
                                let mut status_signal = status_signal;

                                let result = match set_media_hidden(&SetMediaHiddenReq {
                                    media_uuid: media_uuid,
                                    hidden: !media.hidden,
                                }).await {
                                    Ok(_) => String::from("Hidden state updated successfully"),
                                    Err(err) => format!("Error updating hidden state: {}", err.to_string()),

                                };

                                status_signal.set(result);
                            },
                            r#type: "button",
                            "Toggle Hidden"
                        }
                    }
                },
            }
        }
        div {
            class: "modal-footer",
            span { "{status_signal()}" }
        }
    }
}
