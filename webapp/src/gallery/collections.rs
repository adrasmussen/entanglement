use dioxus::prelude::*;
use dioxus_router::prelude::*;
use tracing::error;

use crate::{
    components::modal::{Modal, MODAL_STACK},
    Route,
};
use api::collection::*;
use api::media::MediaUuid;

// no update_signal() because all of the changes are made through modal boxes
#[derive(Clone, PartialEq, Props)]
pub struct CollectionTableProps {
    collection_uuids: Memo<Vec<CollectionUuid>>,
    media_uuid: Memo<MediaUuid>,
}

#[component]
pub fn CollectionTable(props: CollectionTableProps) -> Element {
    rsx! {
        ErrorBoundary {
            handle_error: |error: ErrorContext| {
                rsx! {
                    if let Some(error_ui) = error.show() {
                        {error_ui}
                    } else {
                        div { "CollectionTable encountered an error.  Check the logs or reach out the the administrators." }
                    }
                }
            },
            CollectionTableInner {
                collection_uuids: props.collection_uuids,
                media_uuid: props.media_uuid,
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
struct CollectionTableInnerProps {
    collection_uuids: Memo<Vec<CollectionUuid>>,
    media_uuid: Memo<MediaUuid>,
}

#[component]
fn CollectionTableInner(props: CollectionTableInnerProps) -> Element {
    let collection_uuids = props.collection_uuids;
    let media_uuid = *props.media_uuid.read();

    let collections_future = use_resource(move || {
        async move {
            let mut collections = Vec::new();

            for collection_uuid in collection_uuids() {
                match get_collection(&GetCollectionReq { collection_uuid }).await {
                    Ok(resp) => collections.push((collection_uuid, resp.collection)),
                    Err(err) => {
                        error!("Failed to fetch collection for {collection_uuid}: {err}");
                    }
                }
            }

            // Sort collections by name
            collections.sort_by(|a, b| a.1.name.cmp(&b.1.name));
            collections
        }
    });

    let collections = &*collections_future.read();
    let collections = collections.clone();

    rsx! {
        div {
            class: "detail-section",
            style: "background-color: var(--surface); padding: var(--space-4); border-radius: var(--radius-lg); box-shadow: var(--shadow-sm);",
            div {
                class: "section-header",
                style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: var(--space-3);",
                h2 { "Collections" }
                button {
                    class: "btn btn-secondary",
                    onclick: move |_| {
                        MODAL_STACK.with_mut(|v| v.push(Modal::AddMediaToCollection(media_uuid)));
                    },
                    "Add to Collection"
                }
            }

            match collections {
                Some(collections) => {
                    if collections.is_empty() {
                        rsx! {
                            div {
                                class: "empty-state",
                                style: "padding: var(--space-3); text-align: center; color: var(--text-tertiary);",
                                "This media is not in any collections."
                            }
                        }
                    } else {
                        rsx! {
                            div {
                                class: "table-container",
                                style: "max-height: 250px; overflow-y: auto; border: 1px solid var(--border); border-radius: var(--radius-md);",
                                table { style: "border-collapse: separate; border-spacing: 0;",
                                    thead { style: "position: sticky; top: 0; z-index: 1; background-color: var(--primary);",
                                        tr {
                                            th { "Name" }
                                            th { "Group" }
                                            th { "Note" }
                                            th { style: "width: 100px;", "Actions" }
                                        }
                                    }
                                    tbody {
                                        for (collection_id , collection) in collections.clone() {
                                            tr {
                                                td { style: "padding: var(--space-2) var(--space-3);",
                                                    Link {
                                                        to: Route::CollectionDetail {
                                                            collection_uuid: collection_id.to_string(),
                                                        },
                                                        style: "font-weight: 500; color: var(--primary);",
                                                        "{collection.name}"
                                                    }
                                                }
                                                td { style: "padding: var(--space-2) var(--space-3);", "{collection.gid}" }
                                                td {
                                                    style: "max-width: 200px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; padding: var(--space-2) var(--space-3);",
                                                    title: "{collection.note}",
                                                    if collection.note.is_empty() {
                                                        "(No description)"
                                                    } else {
                                                        "{collection.note}"
                                                    }
                                                }
                                                td { style: "text-align: right; padding: var(--space-2) var(--space-3);",
                                                    button {
                                                        class: "btn btn-sm btn-danger",
                                                        onclick: move |_| {
                                                            MODAL_STACK
                                                                .with_mut(|v| {
                                                                    v.push(Modal::RmMediaFromCollection(media_uuid, collection_id))
                                                                });
                                                        },
                                                        "Remove"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            div { style: "text-align: right; margin-top: var(--space-2); font-size: 0.875rem; color: var(--text-tertiary);",
                                "Showing {collections.len()} collection"
                                if collections.len() != 1 {
                                    "s"
                                } else {
                                    ""
                                }
                            }
                        }
                    }
                }
                None => {
                    rsx! {
                        div { class: "loading-state",
                            for _ in 0..3 {
                                div { class: "skeleton", style: "height: 36px; margin-bottom: 8px;" }
                            }
                        }
                    }
                }
            }
        }
    }
}
