use dioxus::prelude::*;

use crate::{
    common::stream::full_link,
    components::modal::{MODAL_STACK, ModalSize, ModernModal},
};
use api::media::*;

#[derive(Clone, PartialEq, Props)]
pub struct MediaDetailModalProps {
    update_signal: Signal<()>,
    media_uuid: MediaUuid,
}

#[component]
pub fn MediaDetailModal(props: MediaDetailModalProps) -> Element {
    let media_uuid = props.media_uuid;
    let mut update_signal = props.update_signal;

    let status_signal = use_signal(|| String::from(""));

    // Fetch media data
    let media_future =
        use_resource(move || async move { get_media(&GetMediaReq { media_uuid }).await });

    match &*media_future.read() {
        Some(Ok(media_data)) => {
            let mut media = media_data.media.clone();

            // Create footer with action buttons
            let footer = {
                let date = media.date.clone();
                let note = media.date.clone();

                rsx! {
                    span { class: "{status_signal()}" }
                    div { class: "modal-buttons",
                        button {
                            class: "btn btn-secondary",
                            onclick: move |_| {
                                MODAL_STACK.with_mut(|v| v.pop());
                            },
                            "Close"
                        }
                        button {
                            class: "btn btn-primary",
                            onclick: move |_| {
                                let date = date.clone();
                                let note = note.clone();
                                async move {
                                    let mut status_signal = status_signal;
                                    match update_media(
                                            &UpdateMediaReq {
                                                media_uuid,
                                                update: MediaUpdate {
                                                    hidden: Some(media.hidden),
                                                    date: Some(date),
                                                    note: Some(note),
                                                },
                                            },
                                        )
                                        .await
                                    {
                                        Ok(_) => {
                                            status_signal.set("Changes saved successfully".to_string());
                                            update_signal.set(());
                                        }
                                        Err(err) => {
                                            status_signal.set(format!("Error: {}", err));
                                        }
                                    }
                                }
                            },
                            "Save Changes"
                        }
                    }
                }
            };

            return rsx! {
                ModernModal {
                    title: "Media Details",
                    size: ModalSize::Large,
                    footer,

                    div { class: "media-detail-layout",
                        // Left side: Media preview
                        div { class: "media-preview",
                            match media.metadata {
                                MediaMetadata::Image => {
                                    rsx! {
                                        div { class: "fullsize-image-container",
                                            img {
                                                src: full_link(media_uuid),
                                                alt: media.note.clone(),
                                                class: "fullsize-image",
                                                // Optional: Add event listeners for pan/zoom functionality
                                            }
                                        }
                                    }
                                }
                                MediaMetadata::Video => {
                                    rsx! {
                                        video { controls: true, src: full_link(media_uuid) }
                                    }
                                }
                                _ => rsx! {
                                    div { class: "unsupported-media", "Unsupported media type" }
                                },
                            }
                        }

                        // Right side: Media metadata form
                        div { class: "media-metadata",
                            div { class: "form-group",
                                label { class: "form-label", "Path" }
                                input {
                                    class: "form-input",
                                    r#type: "text",
                                    value: "{media.path}",
                                    disabled: true,
                                }
                            }

                            div { class: "form-group",
                                label { class: "form-label", "Library" }
                                input {
                                    class: "form-input",
                                    r#type: "text",
                                    value: "{media.library_uuid}",
                                    disabled: true,
                                }
                            }

                            div { class: "form-group",
                                label { class: "form-label", "Date" }
                                input {
                                    class: "form-input",
                                    r#type: "text",
                                    value: "{media.date.clone()}",
                                    oninput: move |evt| {
                                        media.date = evt.value().clone();
                                    },
                                }
                            }

                            div { class: "form-group",
                                label { class: "form-label", "Note" }
                                textarea {
                                    class: "form-textarea",
                                    rows: 5,
                                    value: "{media.note}",
                                    oninput: move |evt| {
                                        media.note = evt.value().clone();
                                    },
                                }
                            }

                            div { class: "form-group form-checkbox",
                                input {
                                    r#type: "checkbox",
                                    id: "hidden-checkbox",
                                    checked: media.hidden,
                                    oninput: move |evt| {
                                        media.hidden = evt.checked();
                                    },
                                }
                                label { r#for: "hidden-checkbox", "Hidden" }
                            }

                            div { class: "albums-section",
                                h3 { "In Albums" }
                                if media_data.albums.is_empty() {
                                    p { "Not in any albums" }
                                } else {
                                    ul { class: "albums-list",
                                        for album_id in &media_data.albums {
                                            li { "Album #{album_id}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            };
        }
        Some(Err(err)) => {
            return rsx! {
                ModernModal {
                    title: "Error",
                    size: ModalSize::Small,

                    footer: rsx! {
                        button {
                            class: "btn btn-secondary",
                            onclick: move |_| {
                                MODAL_STACK.with_mut(|v| v.pop());
                            },
                            "Close"
                        }
                    },

                    div { class: "error-state",
                        p { "Failed to load media: {err}" }
                    }
                }
            };
        }
        None => {
            return rsx! {
                ModernModal { title: "Loading...", size: ModalSize::Medium,

                    div { class: "skeleton-loader",
                        div { class: "skeleton", style: "height: 300px;" }
                        div {
                            class: "skeleton",
                            style: "height: 24px; width: 80%; margin-top: 16px;",
                        }
                        div {
                            class: "skeleton",
                            style: "height: 24px; width: 60%; margin-top: 8px;",
                        }
                    }
                }
            };
        }
    };
}
