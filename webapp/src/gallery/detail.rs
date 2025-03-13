use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::{
    common::stream::full_link,
    components::modal::{Modal, ModalBox, MODAL_STACK},
    gallery::{albums::AlbumTable, comments::CommentList, similar::SimilarMedia},
    Route,
};
use api::media::*;

#[derive(Clone, PartialEq, Props)]
pub struct GalleryDetailProps {
    // This is a String because we get it from the Router
    media_uuid: String,
}

#[component]
pub fn GalleryDetail(props: GalleryDetailProps) -> Element {
    let update_signal = use_signal(|| ());

    rsx! {
        ModalBox { update_signal }
        ErrorBoundary {
            handle_error: |error: ErrorContext| {
                rsx! {
                    if let Some(error_ui) = error.show() {
                        {error_ui}
                    } else {
                        div { "GalleryDetail encountered an error.  Check the logs or reach out the the administrators." }
                    }
                }
            },
            GalleryInner { update_signal: update_signal, media_uuid: props.media_uuid }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct GalleryErrorProps {
    message: String,
}

#[component]
fn GalleryError(props: GalleryErrorProps) -> Element {
    rsx! {
        div { class: "container error-state",
            h1 { "Error Loading Media" }
            p { "There was an error loading the media: {props.message}" }
            Link { to: Route::GallerySearch {}, class: "btn btn-primary", "Return to Gallery" }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
struct GalleryInnerProps {
    update_signal: Signal<()>,
    media_uuid: String,
}

#[component]
fn GalleryInner(props: GalleryInnerProps) -> Element {
    let mut update_signal = props.update_signal;
    let media_uuid = props.media_uuid.parse::<MediaUuid>().show(|_| {
        let message = "The media_uuid could not be parsed".to_string();
        rsx! {
            GalleryError { message }
        }
    })?;

    //
    let media_uuid = use_memo(use_reactive(&media_uuid, |media_uuid| media_uuid));

    let mut status_signal = use_signal(|| String::from(""));

    // media_uuid is nonreactive, as it comes from the Router
    //
    // thus, to ensure that *this* component re-renders correctly upon changing media URLs directly,
    // we use_reactive() to subscribe the get_media() future to media_uuid as if it were reactive
    let media_future = use_resource(move || async move {
        let media_uuid = *media_uuid.read();
        update_signal.read();

        get_media(&GetMediaReq { media_uuid }).await
    });

    // this subscribes the whole component to the use_resource() reactive variable
    let media_data = &*media_future.read();

    // converting the option to an error and bubbling it up results in the page never reloading, so we
    // transpose to handle the possible error first and then match/return early
    let media_data = match media_data.clone().transpose().show(|error| {
        rsx! {
            GalleryError { message: error }
        }
    })? {
        Some(v) => v,
        None => {
            return rsx! {
                GalleryDetailSkeleton {}
            }
        }
    };

    // extract the parts of the message, and use memos to send them to child elements
    let media = media_data.media;

    // subscribe other elements via memos
    let album_uuids = use_memo(move || match &*media_future.read() {
        Some(Ok(v)) => v.albums.clone(),
        _ => Vec::new(),
    });

    let comment_uuids = use_memo(move || match &*media_future.read() {
        Some(Ok(v)) => v.comments.clone(),
        _ => Vec::new(),
    });

    rsx! {
        div { class: "container",
            // Breadcrumb navigation
            div { class: "breadcrumb",
                Link { to: Route::GallerySearch {}, "Gallery" }
                span { " / " }
                span { "Media Details" }
            }

            // New side-by-side layout with independent scrolling
            div {
                class: "media-detail-page",
                style: "
                    display: flex;
                    gap: var(--space-6);
                    position: relative;
                    height: calc(100vh - 160px);
                ",

                // Left column - Media display (fixed position)
                div {
                    class: "media-detail-main",
                    style: "
                        flex: 0 0 50%;
                        max-width: 50%;
                        position: sticky;
                        top: var(--header-height);
                        height: fit-content;
                        max-height: calc(100vh - 140px);
                        overflow: hidden;
                    ",

                    // Image display
                    div { class: "media-detail-view",
                        match media.metadata {
                            MediaMetadata::Image => rsx! {
                                img {
                                    src: full_link(media_uuid()),
                                    alt: media.note.clone(),
                                    class: "media-detail-image",
                                    style: "
                                        width: 100%;
                                        border-radius: var(--radius-lg);
                                        cursor: pointer;
                                        max-height: calc(100vh - 280px);
                                        object-fit: contain;
                                    ",
                                    onclick: move |_| {
                                        MODAL_STACK.with_mut(|v| v.push(Modal::EnhancedImageView(media_uuid())));
                                    },
                                }
                            },
                            MediaMetadata::Video => rsx! {
                                video {
                                    controls: true,
                                    src: full_link(media_uuid()),
                                    class: "media-detail-video",
                                    style: "
                                        width: 100%;
                                        border-radius: var(--radius-lg);
                                        max-height: calc(100vh - 280px);
                                    ",
                                }
                            },
                            _ => rsx! {
                                div { class: "unsupported-media", "This media type is not supported for preview" }
                            },
                        }
                    }

                    SimilarMedia { media_uuid }
                }

                // Right column - All metadata, albums, and comments (scrollable)
                div {
                    class: "media-detail-sidebar",
                    style: "
                        flex: 1;
                        display: flex;
                        flex-direction: column;
                        gap: var(--space-6);
                        overflow-y: auto;
                        max-height: calc(100vh - 140px);
                        padding-right: var(--space-2);
                    ",

                    // Media metadata form
                    div { class: "detail-section",
                        form {
                            class: "media-detail-form",
                            onsubmit: move |event| async move {
                                let date = event.values().get("date").map(|v| v.as_value());
                                let note = event.values().get("note").map(|v| v.as_value());
                                match update_media(
                                        &UpdateMediaReq {
                                            media_uuid: media_uuid(),
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

                            div {
                                class: "form-row",
                                style: "display: flex; gap: var(--space-4);",
                                div { class: "form-group", style: "flex: 1;",
                                    label { class: "form-label", "Library" }
                                    input {
                                        class: "form-input",
                                        r#type: "text",
                                        value: "{media.library_uuid}",
                                        disabled: true,
                                    }
                                }

                                div { class: "form-group", style: "flex: 1;",
                                    label { class: "form-label", "Path" }
                                    input {
                                        class: "form-input",
                                        r#type: "text",
                                        value: "{media.path}",
                                        disabled: true,
                                    }
                                }
                            }

                            div {
                                class: "form-row",
                                style: "display: flex; gap: var(--space-4); align-items: flex-end;",
                                div { class: "form-group", style: "flex: 1;",
                                    label { class: "form-label", "Date" }
                                    input {
                                        class: "form-input",
                                        name: "date",
                                        r#type: "text",
                                        value: "{media.date}",
                                    }
                                }

                                div {
                                    class: "form-group form-checkbox",
                                    style: "margin-bottom: var(--space-6);",
                                    input {
                                        id: "hidden-checkbox",
                                        r#type: "checkbox",
                                        checked: media.hidden,
                                        onclick: move |_| async move {
                                            match update_media(
                                                    &UpdateMediaReq {
                                                        media_uuid: media_uuid(),
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
                                    rows: 3,
                                    value: "{media.note}",
                                }
                            }

                            div {
                                class: "form-actions",
                                style: "display: flex; align-items: center; gap: var(--space-4);",
                                button {
                                    class: "btn btn-primary",
                                    r#type: "submit",
                                    "Save Changes"
                                }

                                button {
                                    class: "btn btn-secondary",
                                    onclick: move |_| {
                                        let link = full_link(media_uuid());
                                        let window = web_sys::window().expect("no global window exists");
                                        let _ = window.open_with_url_and_target(&link, "_blank");
                                    },
                                    "Download Original"
                                }

                                if !status_signal().is_empty() {
                                    span {
                                        class: "status-message",
                                        style: "color: var(--secondary);",
                                        "{status_signal()}"
                                    }
                                }
                            }
                        }
                    }

                    // Albums section - using our new component
                    AlbumTable { album_uuids, media_uuid }

                    // Use our new comments component
                    CommentList {
                        comment_uuids,
                        media_uuid,
                        update_signal,
                    }

                    // Add some padding at the bottom to ensure good scrolling
                    div { style: "height: var(--space-4);" }
                }
            }
        }
    }
}

#[component]
fn GalleryDetailSkeleton() -> Element {
    rsx! {
        div { class: "container loading-state",
            div {
                class: "skeleton",
                style: "height: 40px; width: 200px; margin-bottom: 16px;",
            }
            div {
                class: "media-detail-page skeleton-layout",
                style: "display: flex; gap: var(--space-6);",

                // Left column skeleton
                div { style: "flex: 0 0 50%;",
                    div {
                        class: "skeleton",
                        style: "height: 400px; margin-bottom: 16px;",
                    }
                }

                // Right column skeleton
                div { style: "flex: 1;",
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
                    div {
                        class: "skeleton",
                        style: "height: 150px; margin-bottom: 16px;",
                    }
                }
            }
        }
    }
}
