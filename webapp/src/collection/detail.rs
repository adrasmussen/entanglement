use std::collections::HashSet;

use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::{
    Route,
    collection::MEDIA_SEARCH_KEY,
    common::{local_time, storage::*},
    components::{
        media_card::MediaCard,
        modal::{MODAL_STACK, Modal, ModalBox},
        search_bar::SearchBar,
    },
};
use api::{
    collection::*,
    fold_set,
    search::{BatchSearchAndSortReq, SearchFilter, SearchRequest, batch_search_and_sort},
    sort::SortMethod,
};

#[derive(Clone, PartialEq, Props)]
pub struct CollectionDetailProps {
    // This is a String because we get it from the Router
    collection_uuid: String,
}

#[component]
pub fn CollectionDetail(props: CollectionDetailProps) -> Element {
    let update_signal = use_signal(|| ());

    rsx! {
        ModalBox { update_signal }
        ErrorBoundary {
            handle_error: |error: ErrorContext| {
                rsx! {
                    if let Some(error_ui) = error.show() {
                        {error_ui}
                    } else {
                        div { "CollectionDetail encountered an error.  Check the logs or reach out the the administrators." }
                    }
                }
            },
            CollectionInner { update_signal, collection_uuid: props.collection_uuid }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct CollectionErrorProps {
    message: String,
}

#[component]
fn CollectionError(props: CollectionErrorProps) -> Element {
    rsx! {
        div { class: "container error-state",
            h1 { "Error Loading Collection" }
            p { "There was an error loading the collection: {props.message}" }
            Link { to: Route::CollectionSearch {}, class: "btn btn-primary", "Return to Collections" }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
struct CollectionInnerProps {
    update_signal: Signal<()>,
    collection_uuid: String,
}

#[component]
fn CollectionInner(props: CollectionInnerProps) -> Element {
    let update_signal = props.update_signal;
    let bulk_edit_mode_signal = use_signal(|| false);
    let selected_media_signal = use_signal(HashSet::new);

    let collection_uuid = props.collection_uuid.parse::<CollectionUuid>().show(|_| {
        let message = "The collection_uuid could not be parsed".to_string();
        rsx! {
            CollectionError { message }
        }
    })?;

    // see GalleryInner for details
    let collection_uuid = use_memo(use_reactive(&collection_uuid, |collection_uuid| {
        collection_uuid
    }));
    let collection_future = use_resource(move || async move {
        let collection_uuid = collection_uuid();
        get_collection(&GetCollectionReq { collection_uuid }).await
    });

    let media_search_signal = use_signal::<String>(|| try_local_storage(MEDIA_SEARCH_KEY));
    let media_future = use_resource(move || async move {
        let collection_uuid = collection_uuid();
        let filter = media_search_signal()
            .split_whitespace()
            .map(|s| s.to_owned())
            .collect();

        batch_search_and_sort(&BatchSearchAndSortReq {
            req: SearchRequest::Collection(SearchMediaInCollectionReq {
                collection_uuid,
                filter: SearchFilter::SubstringAny { filter },
            }),
            sort: SortMethod::Date,
        })
        .await
    });

    // see GalleryInner for details
    //
    // the two futures both early return the same loading skeleton, but they could differ in principle
    let collection_data = &*collection_future.read();
    let collection_data = match collection_data.clone().transpose().show(|error| {
        rsx! {
            CollectionError { message: format!("There was an error fetching the collection metadata: {error}") }
        }
    })? {
        None => {
            return rsx! {
                CollectionDetailSkeleton {}
            }
        }
        Some(v) => v,
    };

    let media_data = &*media_future.read();
    let media_data = match media_data.clone().transpose().show(|error| {
        rsx! {
            CollectionError { message: format!("There was an error searching media in the collection: {error}") }
        }
    })? {
        None => {
            return rsx! {
                CollectionDetailSkeleton {}
            }
        }
        Some(v) => v,
    };

    let collection = collection_data.collection;
    let media = media_data.media;

    let formatted_time = local_time(collection.mtime);
    let formatted_tags = fold_set(collection.tags.clone())
        .unwrap_or_else(|_| "invalid tags, contact admins".to_string());

    rsx! {
        div { class: "container with-sticky",
            ModalBox { update_signal }

            div { class: "sticky-header",
                // breadcrumb navigation
                div {
                    class: "breadcrumb",
                    style: "margin-bottom: var(--space-4);",
                    Link { to: Route::CollectionSearch {}, "Collections" }
                    span { " / " }
                    span { "{collection.name}" }
                }

                // collection detail view header
                div {
                    class: "collection-detail-header",
                    style: "
                        background-color: var(--surface);
                        border-radius: var(--radius-lg);
                        padding: var(--space-4);
                        margin-bottom: var(--space-4);
                        box-shadow: var(--shadow-sm);
                    ",
                    div { style: "display: flex; justify-content: space-between; align-items: flex-start;",
                        // Collection info
                        div {
                            h1 { style: "margin: 0 0 var(--space-2) 0;", "{collection.name}" }
                            div { style: "
                                    display: flex;
                                    gap: var(--space-4);
                                    margin-bottom: var(--space-3);
                                    color: var(--text-secondary);
                                    font-size: 0.875rem;
                                ",
                                span { "Owner: {collection.uid}" }
                                span { "Group: {collection.gid}" }
                                span { "Last modified: {formatted_time}" }
                            }

                            if !collection.note.is_empty() {
                                p { style: "
                                        padding: var(--space-3);
                                        background-color: var(--neutral-50);
                                        border-radius: var(--radius-md);
                                        font-style: italic;
                                        color: var(--text-secondary);
                                        max-width: 700px;
                                    ",
                                    "{collection.note}"
                                }
                            }
                            if !collection.tags.is_empty() {
                                p { style: "
                                        padding: var(--space-3);
                                        background-color: var(--neutral-50);
                                        border-radius: var(--radius-md);
                                        font-style: italic;
                                        color: var(--text-secondary);
                                        max-width: 700px;
                                    ",
                                    "Tags: {formatted_tags}"
                                }
                            }
                        }
                        // Action buttons
                        div { style: "display: flex; gap: var(--space-2);",
                            button {
                                class: "btn btn-secondary",
                                onclick: move |_| {
                                    MODAL_STACK.with_mut(|v| v.push(Modal::EditCollection(collection_uuid())));
                                },
                                "Edit Collection"
                            }
                            button {
                                class: "btn btn-danger",
                                onclick: move |_| {
                                    MODAL_STACK.with_mut(|v| v.push(Modal::DeleteCollection(collection_uuid())));
                                },
                                "Delete Collection"
                            }
                        }
                    }
                }

                SearchBar {
                    search_signal: media_search_signal,
                    storage_key: MEDIA_SEARCH_KEY,
                    placeholder: "Search media in this collection...",
                    status: format!("Found {} items in this collection", media.len()),
                }
            }

            div { class: "scrollable-content",
                // media grid
                if media.is_empty() {
                    div {
                        class: "empty-state",
                        style: "
                            padding: var(--space-8) var(--space-4);
                            text-align: center;
                            background-color: var(--surface);
                            border-radius: var(--radius-lg);
                            margin-top: var(--space-4);
                        ",
                        div { style: "
                                font-size: 4rem;
                                margin-bottom: var(--space-4);
                                color: var(--neutral-400);
                            ",
                            "🖼️"
                        }
                        h3 { style: "
                                margin-bottom: var(--space-2);
                                color: var(--text-primary);
                            ",
                            "No Media in This Collection"
                        }
                        p { style: "
                                color: var(--text-secondary);
                                max-width: 500px;
                                margin: 0 auto;
                            ",
                            "This collection doesn't contain any media yet. Add some media to get started."
                        }
                        button {
                            class: "btn btn-primary",
                            style: "margin-top: var(--space-4);",
                            onclick: move |_| {},
                            "Add Media to Collection"
                        }
                    }
                } else {
                    div {
                        class: "media-grid",
                        style: "
                            display: grid;
                            grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
                            gap: var(--space-4);
                            margin-top: var(--space-4);
                        ",
                        for media in media.iter() {
                            MediaCard {
                                key: "{media.media_uuid}",
                                media_uuid: media.media_uuid,
                                media: media.media.clone(),
                                collections: media.collections.clone(),
                                bulk_edit_mode_signal,
                                selected_media_signal,
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn CollectionDetailSkeleton() -> Element {
    rsx! {
        div { class: "container loading-state",
            div {
                class: "skeleton",
                style: "height: 40px; width: 200px; margin-bottom: 16px;",
            }
            div {
                class: "skeleton",
                style: "height: 200px; margin-bottom: 16px;",
            }
            div {
                class: "skeleton",
                style: "height: 60px; margin-bottom: 16px;",
            }
            div { style: "
                    display: grid;
                    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
                    gap: var(--space-4);
                ",
                for _ in 0..6 {
                    div { class: "skeleton", style: "height: 200px;" }
                }
            }
        }
    }
}
