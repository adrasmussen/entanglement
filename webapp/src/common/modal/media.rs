use dioxus::prelude::*;

use crate::common::{
    modal::{modal_err, Modal, MODAL_STACK},
    stream::*,
};
use api::media::*;

#[derive(Clone, PartialEq, Props)]
pub struct ShowMediaBoxProps {
    media_uuid: MediaUuid,
}

#[component]
pub fn ShowMediaBox(props: ShowMediaBoxProps) -> Element {
    let media_uuid = props.media_uuid;

    let status_signal = use_signal(|| String::from(""));

    let media_future = use_resource(move || async move {
        status_signal();

        get_media(&GetMediaReq {
            media_uuid: media_uuid,
        })
        .await
    });

    let (media, _albums, _comments) = match &*media_future.read() {
        Some(Ok(resp)) => (
            resp.media.clone(),
            resp.albums.clone(),
            resp.comments.clone(),
        ),
        Some(Err(err)) => return modal_err(err.to_string()),
        None => return modal_err("Still waiting on get_media future..."),
    };

    rsx! {
        div { class: "modal-media",
            div {
                img { src: full_link(media_uuid) }
            }
            div {
                form {
                    class: "modal-info",
                    onsubmit: move |event| async move {
                        let mut status_signal = status_signal;
                        let date = event.values().get("date").map(|v| v.as_value());
                        let note = event.values().get("note").map(|v| v.as_value());
                        let result = match update_media(
                                &UpdateMediaReq {
                                    media_uuid: media_uuid.clone(),
                                    update: MediaUpdate {
                                        hidden: None,
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
                    span { "{media.library_uuid}" }

                    label { "Path" }
                    span { "{media.path}" }

                    label { "Hidden" }
                    span { "{media.hidden}" }

                    label { "Date" }
                    input { name: "date", r#type: "text", value: "{media.date}" }

                    label { "Note" }
                    textarea { name: "note", rows: "8", value: "{media.note}" }

                    input { r#type: "submit", value: "Update metadata" }

                    div { grid_column: "2",

                        button {
                            onclick: move |_| {
                                MODAL_STACK.with_mut(|v| v.push(Modal::CreateAlbum));
                            },
                            r#type: "button",
                            "Create album"
                        }
                        button { onclick: move |_| {}, r#type: "button", "Albums" }
                        button {
                            onclick: move |_| async move {
                                let mut status_signal = status_signal;
                                let result = match update_media(
                                        &UpdateMediaReq {
                                            media_uuid: media_uuid,
                                            update: MediaUpdate {
                                                hidden: Some(!media.hidden),
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
                    }
                }
            }
        }
        div { class: "modal-footer",
            span { "{status_signal()}" }
        }
    }
}
