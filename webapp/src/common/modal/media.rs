use dioxus::prelude::*;

use crate::common::{stream::*, modal::Modal};
use api::media::*;

#[derive(Clone, PartialEq, Props)]
pub struct MediaBoxProps {
    stack_signal: Signal<Vec<Modal>>,
    media_uuid: MediaUuid,
}

#[component]
pub fn MediaBox(props: MediaBoxProps) -> Element {
    let _stack_signal = props.stack_signal;
    let media_uuid = props.media_uuid;

    let update_result_signal = use_signal(|| String::from(""));

    let media = use_resource(move || async move {
        update_result_signal();

        get_media(&GetMediaReq {
            media_uuid: media_uuid,
        })
        .await
    });

    let media = &*media.read();

    let result = match media {
        Some(result) => result,
        None => return rsx! {
            div {
                span { "unknown media_uuid {media_uuid}" }
            }
        },
    };

    rsx! {
        div {
            class: "modal-body",
            match result {
                Ok(result) => rsx! {
                    div {
                        img {
                            src: full_link(media_uuid),
                        }
                    }
                    div {
                        form {
                            class: "modal-info",

                            onsubmit: move |event| async move {
                                let mut update_result_signal = update_result_signal;

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

                                update_result_signal.set(result)
                            },

                            label { "Library" },
                            span { "{result.media.library_uuid}" },

                            label { "Path" },
                            span { "{result.media.path}" },

                            label { "Hidden" },
                            span { "{result.media.hidden}" },

                            label { "Date" },
                            input {
                                name: "date",
                                r#type: "text",
                                value: "{result.media.metadata.date}"
                            },

                            label { "Note" },
                            textarea {
                                name: "note",
                                rows: "8",
                                value: "{result.media.metadata.note}"
                            },

                            input {
                                r#type: "submit",
                                value: "Update metadata",
                            },

                            label { "Status" }
                            span {
                                width: "600px",
                                "{update_result_signal()}"
                            }
                        },
                    }
                },
                Err(err) => rsx! {
                    span { "{err.to_string()}" }
                }
            }
        }
    }
}
