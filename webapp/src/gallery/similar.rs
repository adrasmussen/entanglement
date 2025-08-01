use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::Route;
use api::{media::*, thumbnail_link};

#[derive(Clone, PartialEq, Props)]
pub struct SimilarMediaProps {
    media_uuid: Memo<MediaUuid>,
}

#[component]
pub fn SimilarMedia(props: SimilarMediaProps) -> Element {
    rsx! {
        ErrorBoundary {
            handle_error: |error: ErrorContext| {
                rsx! {
                    if let Some(error_ui) = error.show() {
                        {error_ui}
                    } else {
                        div { "SimilarMedia encountered an error.  Check the logs or reach out the the administrators." }
                    }
                }
            },
            SimilarMediaInner { media_uuid: props.media_uuid }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct SimilarMediaInnerProps {
    media_uuid: Memo<MediaUuid>,
}

#[component]
pub fn SimilarMediaInner(props: SimilarMediaInnerProps) -> Element {
    let media_uuid = props.media_uuid;
    let mut distance_signal = use_signal(|| 32);

    let similar_future = use_resource(move || async move {
        let media_uuid = media_uuid();
        let distance = distance_signal();

        similar_media(&SimilarMediaReq {
            media_uuid,
            distance,
        })
        .await
    });

    let similar_media = &*similar_future.read();

    let similar_media = match similar_media.clone().transpose().show(|error| {
        rsx! {
            div { style: "padding: var(--space-4); text-align: center; color: var(--error);",
                "Error loading similar media: {error}"
            }
        }
    })? {
        None => {
            return rsx! {
                div {
                    class: "similar-media-grid skeleton-grid",
                    style: "display: grid; grid-template-columns: repeat(3, 1fr); gap: var(--space-2);",

                    for _ in 0..12 {
                        div {
                            class: "skeleton",
                            style: "border-radius: var(--radius-md); height: 100%;",
                        }
                    }
                }
            };
        },
        Some(v) => v
    };

    let filtered_items = similar_media
        .media
        .iter()
        .filter(|uuid| **uuid != media_uuid())
        .collect::<Vec<_>>();

    rsx! {
        div {
            class: "similar-media-section",
            style: "margin-top: var(--space-4); padding: var(--space-3); background-color: var(--surface); border-radius: var(--radius-lg); box-shadow: var(--shadow-sm);",

            h3 { style: "font-size: 1.125rem; margin-bottom: var(--space-3); display: flex; justify-content: space-between; align-items: center;",
                span { "Similar Media" }

                // Distance selector for similarity threshold
                div { style: "display: flex; align-items: center; gap: var(--space-2);",
                    span { style: "font-size: 0.875rem; font-weight: normal; color: var(--text-tertiary);",
                        "Threshold:"
                    }
                    select {
                        style: "font-size: 0.875rem; padding: 2px 6px; border-radius: var(--radius-md); border: 1px solid var(--border); background-color: var(--surface);
                        ",
                        value: "{distance_signal()}",
                        onchange: move |evt| {
                            if let Ok(val) = evt.value().parse::<i64>() {
                                distance_signal.set(val);
                            }
                        },
                        option { value: "32", "Very Similar" }
                        option { value: "64", "Similar" }
                        option { value: "106", "Somewhat Similar" }
                        option { value: "128", "Broadly Similar" }
                    }
                }
            }
            if filtered_items.is_empty() {
                div { style: "padding: var(--space-4); text-align: center; color: var(--text-tertiary); font-style: italic;",
                    "No similar media found. Try adjusting the threshold."
                }
            } else {

                // Vertical scrollable container for similar media
                div {
                    class: "similar-media-container",
                    style: "
                        overflow-y: auto;
                        max-height: 300px; /* Limit height to enforce scrolling */
                        padding-right: var(--space-2);
                        scroll-behavior: smooth;
                        scrollbar-width: thin;
                        scrollbar-color: var(--neutral-300) transparent;

                        &::-webkit-scrollbar {{
                            width: 6px;
                        }}
                        &::-webkit-scrollbar-track {{
                            background: transparent;
                        }}
                        &::-webkit-scrollbar-thumb {{
                            background-color: var(--neutral-300);
                            border-radius: 20px;
                        }}
                    ",

                    div {
                        class: "similar-media-grid",
                        style: "display: grid; grid-template-columns: repeat(3, 1fr); gap: var(--space-2); width: 100%;",

                        for & media_uuid in filtered_items {
                            Link {
                                key: "{media_uuid}",
                                to: Route::GalleryDetail {
                                    media_uuid: media_uuid.to_string(),
                                },
                                div {
                                    class: "similar-media-item",
                                    style: "
                                        position: relative;
                                        overflow: hidden;
                                        border-radius: var(--radius-md);
                                        height: 100%;
                                        transition: transform var(--transition-fast) var(--easing-standard);

                                        &:hover {{
                                            transform: scale(1.05);
                                            box-shadow: var(--shadow-md);
                                            z-index: 1;
                                        }}
                                    ",
                                    img {
                                        src: thumbnail_link(media_uuid),
                                        alt: "Similar media",
                                        style: "width: 100%; aspect-ratio: 1; object-fit: cover;",
                                        loading: "lazy",
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
