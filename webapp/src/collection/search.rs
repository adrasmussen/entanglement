// webapp/src/collection/search.rs

use dioxus::prelude::*;

use crate::{
    collection::{grid::CollectionGrid, COLLECTION_SEARCH_KEY},
    common::storage::*,
    components::{
        modal::{Modal, ModalBox, MODAL_STACK},
        search_bar::SearchBar,
    },
};
use api::{collection::*, search::SearchFilter};

#[component]
pub fn CollectionSearch() -> Element {
    let update_signal = use_signal(|| ());

    // Get search signal from local storage
    let collection_search_signal =
        use_signal::<String>(|| try_local_storage(COLLECTION_SEARCH_KEY));

    // Fetch collections data
    let collection_future = use_resource(move || async move {
        // Read the update signal to trigger a refresh when needed
        update_signal.read();
        let filter = collection_search_signal()
            .split_whitespace()
            .map(|s| s.to_owned())
            .collect();

        search_collections(&SearchCollectionsReq {
            filter: SearchFilter::SubstringAny { filter },
        })
        .await
    });

    // Create action button for search bar - positioned on the right
    let action_button = rsx! {
        div { style: "margin-left: auto;", // This will push the button to the right
            button {
                class: "btn btn-primary",
                onclick: move |_| {
                    MODAL_STACK.with_mut(|v| v.push(Modal::CreateCollection));
                },
                "Create Collection"
            }
        }
    };

    // Get status text
    let status = match &*collection_future.read() {
        Some(Ok(resp)) => format!("Found {} collections", resp.collections.len()),
        Some(Err(_)) => String::from("Error searching collections"),
        None => String::from("Loading..."),
    };

    rsx! {
        div { class: "container",
            // Modal container for popups
            ModalBox { update_signal }

            // Page header
            div { class: "page-header", style: "margin-bottom: var(--space-4);",
                h1 { class: "section-title", "Collections" }
                p { "Organize and browse your media collections" }
            }

            // Search controls
            SearchBar {
                search_signal: collection_search_signal,
                storage_key: COLLECTION_SEARCH_KEY,
                placeholder: "Search by collection name or description...",
                status,
                action_button,
            }

            // Collection grid
            match &*collection_future.read() {
                Some(Ok(resp)) => {
                    rsx! {
                        CollectionGrid { collections: resp.collections.clone() }
                    }
                }
                Some(Err(err)) => rsx! {
                    div {
                        class: "error-state",
                        style: "
                            padding: var(--space-4);
                            background-color: var(--surface);
                            border-radius: var(--radius-lg);
                            margin-top: var(--space-4);
                            color: var(--error);
                            text-align: center;
                        ",
                        "Error: {err}"
                    }
                },
                None => rsx! {
                    div {
                        class: "loading-state collections-grid",
                        style: "
                            display: grid;
                            grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
                            gap: var(--space-4);
                            margin-top: var(--space-4);
                        ",
                        for _ in 0..6 {
                            div {
                                class: "collection-card loading",
                                style: "
                                    background-color: var(--surface);
                                    border-radius: var(--radius-lg);
                                    overflow: hidden;
                                    box-shadow: var(--shadow-sm);
                                    height: 100%;
                                ",
                                // Skeleton loading UI
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
                },
            }
        }
    }
}
