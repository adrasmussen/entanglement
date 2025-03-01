use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::{
    common::modal::{Modal, MODAL_STACK},
    Route,
};
use api::media::*;

#[derive(Clone, PartialEq, Props)]
pub struct MediaInfoProps {
    update_signal: Signal<()>,
    media_uuid: MediaUuid,
    media: Media,
}

#[component]
pub fn MediaInfo(props: MediaInfoProps) -> Element {
    let mut update_signal = props.update_signal;
    let media_uuid = props.media_uuid;
    let media = props.media;

    let status_signal = use_signal(|| String::from(""));

    // in the functions below, update_signal.set(()) is called on both success
    // and failure of the underlying API call.  this is so that, in the event
    // of a database issue, we pull the most recent data for the media
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
                    update_signal.set(());
                    status_signal.set(result)
                },

                label { "Library" }
                Link {
                    to: Route::LibraryDetail {
                        library_uuid: media.library_uuid.to_string(),
                    },
                    span { "{media.library_uuid}" }
                }

                label { "Path" }
                span { "{media.path}" }

                label { "Hidden" }
                span { "{media.hidden}" }

                label { "Date" }
                input { name: "date", r#type: "text", value: "{media.date}" }

                label { "Note" }
                textarea { name: "note", rows: "8", value: "{media.note}" }

                input { r#type: "submit", value: "Update metadata" }

                // inside the form because we want to use the grid column created by the labels
                //
                // adjust the template_columns as necessary as buttons are added/removed
                div {
                    grid_column: 2,
                    display: "grid",
                    grid_template_columns: "1fr 1fr 1fr",
                    button {
                        onclick: move |_| { MODAL_STACK.with_mut(|v| v.push(Modal::AddMediaToAnyAlbum(media_uuid))) },
                        r#type: "button",
                        "Add to album"
                    }
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
                            update_signal.set(());
                            status_signal.set(result);
                        },
                        r#type: "button",
                        "Toggle Hidden"
                    }
                }

                div { grid_column: 2,
                    span { {status_signal} }
                }
            }
        }
    }
}
