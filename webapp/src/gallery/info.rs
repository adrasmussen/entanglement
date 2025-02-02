use dioxus::prelude::*;

use api::media::*;

#[derive(Clone, PartialEq, Props)]
pub struct MediaInfoProps {
    media_uuid: MediaUuid,
    media: Media,
    status_signal: Signal<String>,
}

#[component]
pub fn MediaInfo(props: MediaInfoProps) -> Element {
    let media_uuid = props.media_uuid.clone();
    let media = props.media.clone();
    let status_signal = props.status_signal.clone();

    rsx! {
        div { class: "gallery-info",
            form {
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
                span { "{media.library_uuid}" }

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
            }
        }
        div { class: "gallery-info", grid_column: "2",
            button { onclick: move |_| {}, r#type: "button", "Create comment" }
            button { onclick: move |_| {}, r#type: "button", "Add to album" }
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
        // media-specific options go here
    }
}
