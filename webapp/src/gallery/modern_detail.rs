use dioxus::prelude::*;
use dioxus_router::prelude::*;
use web_sys::window;

use crate::{
    common::{
        local_time,
        modal::{Modal, ModalBox, MODAL_STACK},
        stream::full_link,
    },
    components::search_bar::SearchBar,
    Route,
};
use api::media::*;

#[derive(Clone, PartialEq, Props)]
pub struct ModernGalleryDetailProps {
    // This is a String because we get it from the Router
    media_uuid: String,
}

#[component]
pub fn ModernGalleryDetail(props: ModernGalleryDetailProps) -> Element {
    let media_uuid = match props.media_uuid.parse::<MediaUuid>() {
        Ok(uuid) => uuid,
        Err(_) => {
            return rsx! {
                div { class: "error-state container",
                    h1 { "Invalid Media ID" }
                    p { "The provided media ID could not be parsed." }
                    Link {
                        to: Route::ModernGallerySearch {},
                        class: "btn btn-primary",
                        "Return to Gallery"
                    }
                }
            }
        }
    };

    let mut update_signal = use_signal(|| ());

    // Fetch media data
    let media_future = use_resource(move || async move {
        update_signal.read();
        get_media(&GetMediaReq { media_uuid }).await
    });

    let out = match &*media_future.read() {
        Some(Ok(media_data)) => {
            let media = media_data.media.clone();
            let albums = media_data.albums.clone();
            let comments = media_data.comments.clone();

            // Format metadata for display
            let date_formatted = if media.date.is_empty() {
                "Unknown date".to_string()
            } else {
                media.date.clone()
            };

            let mut status_signal = use_signal(|| String::from(""));

            rsx! {
                div { class: "container",
                    ModalBox { update_signal }

                    // Breadcrumb navigation
                    div { class: "breadcrumb",
                        Link { to: Route::ModernGallerySearch {}, "Gallery" }
                        span { " / " }
                        span { "Media Details" }
                    }

                    div { class: "media-detail-page",
                        // Left column - Media display & info
                        div { class: "media-detail-main",
                            // Image display
                            div { class: "media-detail-view",
                                match media.metadata {
                                    MediaMetadata::Image => rsx! {
                                        div { class: "media-detail-view",
                                            img {
                                                src: full_link(media_uuid),
                                                alt: media.note.clone(),
                                                class: "media-detail-image",
                                                onclick: move |_| {
                                                    MODAL_STACK.with_mut(|v| v.push(Modal::EnhancedImageView(media_uuid)));
                                                },
                                            }
                                            div { class: "image-controls",
                                                span { "Click image for full view" }
                                                button {
                                                    class: "btn btn-sm btn-secondary",
                                                    onclick: move |_| {
                                                        let link = full_link(media_uuid);
                                                        let window = web_sys::window().expect("no global window exists");
                                                        let _ = window
                                                            .open_with_url_and_target(&format!("{}?download=true", link), "_blank");
                                                    },
                                                    "Download Original"
                                                }
                                            }
                                        }
                                    },
                                    MediaMetadata::Video => rsx! {
                                        div { class: "media-detail-view",
                                            video {
                                                controls: true,
                                                src: full_link(media_uuid),
                                                class: "media-detail-video",
                                            }
                                            div { class: "image-controls",
                                                button {
                                                    class: "btn btn-sm btn-secondary",
                                                    onclick: move |_| {
                                                        MODAL_STACK.with_mut(|v| v.push(Modal::EnhancedImageView(media_uuid)));
                                                    },
                                                    "Open Fullscreen"
                                                }
                                                button {
                                                    class: "btn btn-sm btn-secondary",
                                                    onclick: move |_| {
                                                        let link = full_link(media_uuid);
                                                        let window = web_sys::window().expect("no global window exists");
                                                        let _ = window
                                                            .open_with_url_and_target(&format!("{}?download=true", link), "_blank");
                                                    },
                                                    "Download Original"
                                                }
                                            }
                                        }
                                    },
                                    _ => rsx! {
                                        div { class: "unsupported-media", "This media type is not supported for preview" }
                                    },
                                }
                            }

                            // Media metadata form
                            form {
                                class: "media-detail-form",
                                onsubmit: move |event| async move {
                                    let date = event.values().get("date").map(|v| v.as_value());
                                    let note = event.values().get("note").map(|v| v.as_value());
                                    match update_media(
                                            &UpdateMediaReq {
                                                media_uuid,
                                                update: MediaUpdate {
                                                    hidden: None,
                                                    date,
                                                    note,
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
                                },

                                h2 { "Media Information" }

                                div { class: "form-row",
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
                                        label { class: "form-label", "Path" }
                                        input {
                                            class: "form-input",
                                            r#type: "text",
                                            value: "{media.path}",
                                            disabled: true,
                                        }
                                    }
                                }

                                div { class: "form-row",
                                    div { class: "form-group",
                                        label { class: "form-label", "Date" }
                                        input {
                                            class: "form-input",
                                            name: "date",
                                            r#type: "text",
                                            value: "{date_formatted}",
                                        }
                                    }

                                    div { class: "form-group form-checkbox",
                                        input {
                                            id: "hidden-checkbox",
                                            r#type: "checkbox",
                                            checked: media.hidden,
                                            onclick: move |_| async move {
                                                match update_media(
                                                        &UpdateMediaReq {
                                                            media_uuid,
                                                            update: MediaUpdate {
                                                                hidden: Some(!media.hidden),
                                                                date: None,
                                                                note: None,
                                                            },
                                                        },
                                                    )
                                                    .await
                                                {
                                                    Ok(_) => {
                                                        status_signal.set("Visibility updated".to_string());
                                                        update_signal.set(());
                                                    }
                                                    Err(err) => {
                                                        status_signal.set(format!("Error: {}", err));
                                                    }
                                                }
                                            },
                                        }
                                        label { r#for: "hidden-checkbox", "Hidden" }
                                    }
                                }

                                div { class: "form-group",
                                    label { class: "form-label", "Note" }
                                    textarea {
                                        class: "form-textarea",
                                        name: "note",
                                        rows: 4,
                                        value: "{media.note}",
                                    }
                                }

                                div { class: "form-actions",
                                    button {
                                        class: "btn btn-primary",
                                        r#type: "submit",
                                        "Save Changes"
                                    }

                                    if !status_signal().is_empty() {
                                        span { class: "status-message", "{status_signal()}" }
                                    }
                                }
                            }
                        }

                        // Right column - Related content
                        div { class: "media-detail-sidebar",
                            // Albums section
                            div { class: "detail-section",
                                div { class: "section-header",
                                    h2 { "Albums" }
                                    button {
                                        class: "btn btn-sm btn-secondary",
                                        onclick: move |_| {
                                            MODAL_STACK.with_mut(|v| v.push(Modal::AddMediaToAnyAlbum(media_uuid)));
                                        },
                                        "Add to Album"
                                    }
                                }

                                if albums.is_empty() {
                                    div { class: "empty-state", "This media is not in any albums." }
                                } else {
                                    div { class: "albums-list",
                                        for album_id in albums {
                                            div { class: "album-item",
                                                Link {
                                                    to: Route::AlbumDetail {
                                                        album_uuid: album_id.to_string(),
                                                    },
                                                    "Album #{album_id}"
                                                }
                                                button {
                                                    class: "btn btn-sm btn-danger",
                                                    onclick: move |_| {
                                                        MODAL_STACK.with_mut(|v| v.push(Modal::RmMediaFromAlbum(media_uuid, album_id)));
                                                    },
                                                    "Remove"
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // Comments section
                            div { class: "detail-section",
                                div { class: "section-header",
                                    h2 { "Comments" }
                                    button {
                                        class: "btn btn-sm btn-secondary",
                                        onclick: move |_| {
                                            MODAL_STACK.with_mut(|v| v.push(Modal::AddComment(media_uuid)));
                                        },
                                        "Add Comment"
                                    }
                                }

                                if comments.is_empty() {
                                    div { class: "empty-state", "No comments yet." }
                                } else {
                                    div { class: "comments-list" }
                                }
                            }
                        }
                    }
                }
            }
        }
        Some(Err(err)) => {
            rsx! {
                div { class: "container error-state",
                    h1 { "Error Loading Media" }
                    p { "There was an error loading the media: {err}" }
                    Link {
                        to: Route::ModernGallerySearch {},
                        class: "btn btn-primary",
                        "Return to Gallery"
                    }
                }
            }
        }
        None => {
            rsx! {
                div { class: "container loading-state",
                    div {
                        class: "skeleton",
                        style: "height: 40px; width: 200px; margin-bottom: 16px;",
                    }
                    div { class: "media-detail-page skeleton-layout",
                        div {
                            class: "skeleton",
                            style: "height: 400px; margin-bottom: 16px;",
                        }
                        div {
                            class: "skeleton",
                            style: "height: 24px; width: 80%; margin-bottom: 8px;",
                        }
                        div {
                            class: "skeleton",
                            style: "height: 24px; width: 60%; margin-bottom: 8px;",
                        }
                        div {
                            class: "skeleton",
                            style: "height: 100px; margin-bottom: 16px;",
                        }
                    }
                }
            }
        }
    };

    out
}
