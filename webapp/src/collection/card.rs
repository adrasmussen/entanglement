use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::Route;
use api::collection::*;

#[derive(Clone, PartialEq, Props)]
pub struct CollectionCardProps {
    collection_uuid: CollectionUuid,
}

#[component]
pub fn CollectionCard(props: CollectionCardProps) -> Element {
    rsx! {
        ErrorBoundary {
            handle_error: |error: ErrorContext| {
                rsx! {
                    if let Some(error_ui) = error.show() {
                        {error_ui}
                    } else {
                        div { "CollectionCard encountered an error.  Check the logs or reach out the the administrators." }
                    }
                }
            },
            CollectionCardInner { collection_uuid: props.collection_uuid }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
struct CollectionCardErrorProps {
    collection_uuid: CollectionUuid,
    message: String,
}

#[component]
fn CollectionCardError(props: CollectionCardErrorProps) -> Element {
    rsx! {
        div {
            class: "collection-card error",
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
            "Error loading collection {props.collection_uuid}: {props.message}"
        }
    }
}

#[derive(Clone, PartialEq, Props)]
struct CollectionCardInnerProps {
    collection_uuid: CollectionUuid,
}

#[component]
fn CollectionCardInner(props: CollectionCardProps) -> Element {
    let collection_uuid = props.collection_uuid;

    // Fetch collection data
    let collection = use_resource(move || async move {
        get_collection(&GetCollectionReq {
            collection_uuid: collection_uuid,
        })
        .await
    });

    let collection_data = &*collection.read();

    let collection_data = match collection_data.clone().transpose().show(|error| {
        rsx! {
            CollectionCardError { collection_uuid, message: error }
        }
    })? {
        Some(v) => v,
        None => {
            return rsx! {
                CollectionCardSkeleton {}
            }
        }
    };

    let collection = collection_data.collection;

    rsx! {
        div {
            class: "collection-card",
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
                to: Route::CollectionDetail {
                    collection_uuid: collection_uuid.to_string(),
                },
                div {
                    class: "collection-thumbnail",
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

                    // placeholder emoji -- eventually this will be collection.cover or similar
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
                class: "collection-info",
                style: "
                    padding: var(--space-3);
                    flex-grow: 1;
                    display: flex;
                    flex-direction: column;
                ",

                Link {
                    to: Route::CollectionDetail {
                        collection_uuid: collection_uuid.to_string(),
                    },
                    h3 {
                        class: "collection-title",
                        style: "
                            margin: 0 0 var(--space-1) 0;
                            font-size: 1.125rem;
                            font-weight: 600;
                            color: var(--text-primary);
                        ",
                        "{collection.name}"
                    }
                }

                div {
                    class: "collection-metadata",
                    style: "
                        display: flex;
                        justify-content: space-between;
                        margin-bottom: var(--space-1);
                        font-size: 0.875rem;
                        color: var(--text-tertiary);
                    ",
                    span { "Owner: {collection.uid}" }
                    span { "Group: {collection.gid}" }
                }

                p {
                    class: "collection-note",
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
                    if collection.note.is_empty() {
                        "(No description)"
                    } else {
                        "{collection.note}"
                    }
                }
            }
        }
    }
}

#[component]
fn CollectionCardSkeleton() -> Element {
    rsx! {
        div {
            class: "collection-card loading",
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
