use dioxus::prelude::*;

use crate::common::style;
use api::media::*;

#[derive(Clone, PartialEq, Props)]
pub struct MediaTileProps {
    pub view_media_signal: Signal<Option<MediaUuid>>,
    pub media_uuid: MediaUuid,
}

#[component]
pub fn MediaTile(props: MediaTileProps) -> Element {
    let mut view_media_signal = props.view_media_signal;
    let media_uuid = props.media_uuid;

    rsx! {
        style { "{style::MEDIA_GRID}" }
        div {
            class: "media-tile",
            img {
                src: "/entanglement/media/thumbnails/{props.media_uuid}",
                onclick: move |_| { view_media_signal.set(Some(media_uuid)) }
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct MediaBoxProps {
    pub view_media_signal: Signal<Option<MediaUuid>>,
}

#[component]
pub fn MediaBox(props: MediaBoxProps) -> Element {
    // generalize these two lines to call each type of modal box,
    // or nothing if the stack is empty
    let mut view_media_signal = props.view_media_signal;

    let media_uuid = match view_media_signal() {
        Some(val) => val,
        None => return rsx! {},
    };

    // everything below here is specific to media

    // the internal signal used to re-render and provide info about a metadata update
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
        None => return rsx! {},
    };



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
                            onclick: move |_| {view_media_signal.set(None)},
                            "X"
                        }
                    }
                    div {
                        class: "modal-body",
                        match result {
                            Ok(result) => rsx! {
                                div {
                                    img {
                                        src: "/entanglement/media/full/{media_uuid}",
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
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct MediaGridProps {
    media: Result<Vec<MediaUuid>, String>,
}

#[component]
pub fn MediaGrid(props: MediaGridProps) -> Element {
    let view_media_signal = use_signal::<Option<MediaUuid>>(|| None);

    rsx! {
        MediaBox{ view_media_signal: view_media_signal }

        div {
            style { "{style::MEDIA_GRID}" }
            match props.media {
                Ok(media) => rsx! {
                    div {
                        class: "media-grid",
                        for media_uuid in media.iter() {
                            MediaTile { view_media_signal: view_media_signal, media_uuid: *media_uuid }
                        }
                    }
                },
                Err(err) => rsx! {
                    span { "{err}" }
                }
            }
        }
    }
}
