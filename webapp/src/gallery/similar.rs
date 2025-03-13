use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::{common::stream::thumbnail_link, Route};
use api::media::*;

#[derive(Clone, PartialEq, Props)]
pub struct SimilarMediaProps {
    media_uuid: MediaUuid,
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
    media_uuid: MediaUuid,
}

#[component]
pub fn SimilarMediaInner(props: SimilarMediaInnerProps) -> Element {
    let media_uuid = props.media_uuid;
    let mut distance_signal = use_signal(|| 64);

    let similar_future = use_resource(move || async move {
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
            div { style: "
                    padding: var(--space-4);
                    text-align: center;
                    color: var(--error);
                ",
                "Error loading similar media: {error}"
            }
        }
    })? {
        Some(v) => v,
        None => {
            return rsx! {
                div {
                    class: "similar-media-grid skeleton-grid",
                    style: "
                        display: grid;
                        grid-template-columns: repeat(3, 1fr);
                        gap: var(--space-2);
                    ",

                    for _ in 0..12 {
                        div {
                            class: "skeleton",
                            style: "
                                border-radius: var(--radius-md);
                                height: 100%;
                            ",
                        }
                    }
                }
            }
        }
    };

    let filtered_items = similar_media
        .media
        .iter()
        .filter(|uuid| **uuid != media_uuid)
        .collect::<Vec<_>>();

    rsx! {
        div {
            class: "similar-media-section",
            style: "
                margin-top: var(--space-4);
                padding: var(--space-3);
                background-color: var(--surface);
                border-radius: var(--radius-lg);
                box-shadow: var(--shadow-sm);
            ",

            h3 { style: "
                    font-size: 1.125rem;
                    margin-bottom: var(--space-3);
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                ",
                span { "Similar Media" }

                // Distance selector for similarity threshold
                div { style: "display: flex; align-items: center; gap: var(--space-2);",
                    span { style: "font-size: 0.875rem; font-weight: normal; color: var(--text-tertiary);",
                        "Threshold:"
                    }
                    select {
                        style: "
                            font-size: 0.875rem;
                            padding: 2px 6px;
                            border-radius: var(--radius-md);
                            border: 1px solid var(--border);
                            background-color: var(--surface);
                        ",
                        value: "{distance_signal()}",
                        onchange: move |evt| {
                            if let Ok(val) = evt.value().parse::<i64>() {
                                distance_signal.set(val);
                            }
                        },
                        option { value: "64", "Very Similar" }
                        option { value: "128", "Similar" }
                        option { value: "192", "Somewhat Similar" }
                        option { value: "256", "Broadly Similar" }
                    }
                }
            }
            if filtered_items.is_empty() {
                div { style: "
                padding: var(--space-4);
                text-align: center;
                color: var(--text-tertiary);
                font-style: italic;
            ",
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
                    /* Enable smooth scrolling */
                    scroll-behavior: smooth;
                    /* Hide scrollbar but keep functionality */
                    scrollbar-width: thin;
                    scrollbar-color: var(--neutral-300) transparent;

                    /* Custom scrollbar styling */
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
                        style: "
                        display: grid;
                        grid-template-columns: repeat(3, 1fr);
                        gap: var(--space-2);
                        width: 100%;
                    ",

                        for & media_id in filtered_items {
                            Link {
                                key: "{media_id}",
                                to: Route::GalleryDetail {
                                    media_uuid: media_id.to_string(),
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
                                        src: thumbnail_link(media_id),
                                        alt: "Similar media",
                                        style: "
                                        width: 100%;
                                        aspect-ratio: 1;
                                        object-fit: cover;
                                    ",
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
