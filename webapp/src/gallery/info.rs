use dioxus::prelude::*;

use crate::common::modal::{Modal, MODAL_STACK};
use api::media::*;

#[derive(Clone, PartialEq, Props)]
pub struct MediaInfoProps {
    media_uuid: MediaUuid,
    media: Media,
    status_signal: Signal<String>,
}

#[component]
pub fn MediaInfo(props: MediaInfoProps) -> Element {
    let media_uuid = props.media_uuid;
    let media = props.media;
    let status_signal = props.status_signal;

    rsx! {
        div {
            form {
                class: "gallery-info",
                onsubmit: move |event| async move {
                    let mut status_signal = status_signal;
                    let date = event.values().get("date").map(|v| v.as_value());
                    let note = event.values().get("note").map(|v| v.as_value());
                    let result = match update_media(
                            &UpdateMediaReq {
                                media_uuid: media_uuid.clone(),
                                update: MediaUpdate {
                                    hidden: None,
                                    attention: None,
                                    date: date,
                                    note: note,
                                },
                            },
                        )
                        .await
                    {
                        Ok(_) => String::from("Metadata updated successfully"),
                        Err(err) => format!("Error updating metadata: {}", err.to_string()),
                    };
                    status_signal.set(result)
                },

                label { "Library" }
                span { "{media.library_uuid} (should include link)" }

                label { "Path" }
                span { "{media.path}" }

                label { "Hidden" }
                span { "{media.hidden}" }

                label { "Needs attention" }
                span { "{media.attention}" }

                label { "Date" }
                input { name: "date", r#type: "text", value: "{media.date}" }

                label { "Note" }
                textarea { name: "note", rows: "8", value: "{media.note}" }

                input { r#type: "submit", value: "Update metadata" }

                // inside the form because we want to use the grid created by the labels
                div { grid_column: 2,
                    button { onclick: move |_| {}, r#type: "button", "Add to album" }
                    button {
                        onclick: move |_| { MODAL_STACK.with_mut(|v| v.push(Modal::AddComment(media_uuid))) },
                        r#type: "button",
                        "Create comment"
                    }
                    button {
                        onclick: move |_| async move {
                            let mut status_signal = status_signal;
                            let result = match update_media(
                                    &UpdateMediaReq {
                                        media_uuid: media_uuid,
                                        update: MediaUpdate {
                                            hidden: Some(!media.hidden),
                                            attention: None,
                                            date: None,
                                            note: None,
                                        },
                                    },
                                )
                                .await
                            {
                                Ok(_) => String::from("Hidden state updated successfully"),
                                Err(err) => format!("Error updating hidden state: {}", err.to_string()),
                            };
                            status_signal.set(result);
                        },
                        r#type: "button",
                        "Toggle Hidden"
                    }
                    button {
                        onclick: move |_| async move {
                            let mut status_signal = status_signal;
                            let result = match update_media(
                                    &UpdateMediaReq {
                                        media_uuid: media_uuid,
                                        update: MediaUpdate {
                                            hidden: None,
                                            attention: Some(!media.attention),
                                            date: None,
                                            note: None,
                                        },
                                    },
                                )
                                .await
                            {
                                Ok(_) => String::from("Attention state updated successfully"),
                                Err(err) => format!("Error updating attention state: {}", err.to_string()),
                            };
                            status_signal.set(result);
                        },
                        r#type: "button",
                        "Needs attention"
                    }
                }
            }
        }
    }
}
