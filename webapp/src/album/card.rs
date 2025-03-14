use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::Route;
use api::album::*;

#[derive(Clone, PartialEq, Props)]
pub struct AlbumCardProps {
    album_uuid: AlbumUuid,
}

#[component]
pub fn AlbumCard(props: AlbumCardProps) -> Element {
    rsx! {
        ErrorBoundary {
            handle_error: |error: ErrorContext| {
                rsx! {
                    if let Some(error_ui) = error.show() {
                        {error_ui}
                    } else {
                        div { "AlbumCard encountered an error.  Check the logs or reach out the the administrators." }
                    }
                }
            },
            AlbumCardInner { album_uuid: props.album_uuid }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
struct AlbumCardErrorProps {
    album_uuid: AlbumUuid,
    message: String,
}

#[component]
fn AlbumCardError(props: AlbumCardErrorProps) -> Element {
    rsx! {
        div {
            class: "album-card error",
            style: "
                background-color: var(--surface);
                border-radius: var(--radius-lg);
                padding: var(--space-3);
                box-shadow: var(--shadow-sm);
                height: 100%;
                display: flex;
                flex-direction: column;
                align-items: center;
                justify-content: center;
                color: var(--error);
            ",
            "Error loading album {props.album_uuid}: {props.message}"
        }
    }
}

#[derive(Clone, PartialEq, Props)]
struct AlbumCardInnerProps {
    album_uuid: AlbumUuid,
}

#[component]
fn AlbumCardInner(props: AlbumCardProps) -> Element {
    let album_uuid = props.album_uuid;

    // Fetch album data
    let album = use_resource(move || async move {
        get_album(&GetAlbumReq {
            album_uuid: album_uuid,
        })
        .await
    });

    let album_data = &*album.read();

    let album_data = match album_data.clone().transpose().show(|error| {
        rsx! {
            AlbumCardError { album_uuid, message: error }
        }
    })? {
        Some(v) => v,
        None => {
            return rsx! {
                AlbumCardSkeleton {}
            }
        }
    };

    let album = album_data.album;

    rsx! {
        div {
            class: "album-card",
            style: "
                background-color: var(--surface);
                border-radius: var(--radius-lg);
                overflow: hidden;
                box-shadow: var(--shadow-sm);
                transition: transform var(--transition-normal) var(--easing-standard),
                            box-shadow var(--transition-normal) var(--easing-standard);
                height: 100%;
                display: flex;
                flex-direction: column;
            ",
            Link {
                to: Route::AlbumDetail {
                    album_uuid: album_uuid.to_string(),
                },
                div {
                    class: "album-thumbnail",
                    style: "
                        height: 180px;
                        background-color: var(--neutral-200);
                        display: flex;
                        align-items: center;
                        justify-content: center;
                        font-size: 2.5rem;
                        color: var(--neutral-500);
                        transition: background-color var(--transition-normal) var(--easing-standard);
                    ",

                    // placeholder emoji -- eventually this will be album.cover or similar
                    div { style: "
                            width: 64px;
                            height: 64px;
                            border-radius: var(--radius-lg);
                            background-color: var(--surface-raised);
                            display: flex;
                            align-items: center;
                            justify-content: center;
                        ",
                        "🖼️"
                    }
                }
            }

            div {
                class: "album-info",
                style: "
                    padding: var(--space-3);
                    flex-grow: 1;
                    display: flex;
                    flex-direction: column;
                ",

                Link {
                    to: Route::AlbumDetail {
                        album_uuid: album_uuid.to_string(),
                    },
                    h3 {
                        class: "album-title",
                        style: "
                            margin: 0 0 var(--space-1) 0;
                            font-size: 1.125rem;
                            font-weight: 600;
                            color: var(--text-primary);
                        ",
                        "{album.name}"
                    }
                }

                div {
                    class: "album-metadata",
                    style: "
                        display: flex;
                        justify-content: space-between;
                        margin-bottom: var(--space-1);
                        font-size: 0.875rem;
                        color: var(--text-tertiary);
                    ",
                    span { "Owner: {album.uid}" }
                    span { "Group: {album.gid}" }
                }

                p {
                    class: "album-note",
                    style: "
                        margin: var(--space-2) 0;
                        font-size: 0.875rem;
                        color: var(--text-secondary);
                        flex-grow: 1;
                        overflow: hidden;
                        display: -webkit-box;
                        -webkit-line-clamp: 2;
                        -webkit-box-orient: vertical;
                    ",
                    if album.note.is_empty() {
                        "(No description)"
                    } else {
                        "{album.note}"
                    }
                }
            }
        }
    }
}

#[component]
fn AlbumCardSkeleton() -> Element {
    rsx! {
        div {
            class: "album-card loading",
            style: "
                background-color: var(--surface);
                border-radius: var(--radius-lg);
                overflow: hidden;
                box-shadow: var(--shadow-sm);
                height: 100%;
            ",
            div { class: "skeleton", style: "height: 180px;" }

            div { style: "padding: var(--space-3);",

                div {
                    class: "skeleton",
                    style: "width: 70%; height: 24px; margin-bottom: var(--space-2);",
                }

                div {
                    class: "skeleton",
                    style: "width: 100%; height: 16px; margin-bottom: var(--space-1);",
                }

                div {
                    class: "skeleton",
                    style: "width: 90%; height: 16px; margin-bottom: var(--space-3);",
                }

                div { style: "display: flex; justify-content: flex-end; gap: var(--space-2);",
                    div {
                        class: "skeleton",
                        style: "width: 40px; height: 24px;",
                    }
                    div {
                        class: "skeleton",
                        style: "width: 40px; height: 24px;",
                    }
                }
            }
        }
    }
}
