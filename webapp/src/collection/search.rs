use dioxus::prelude::*;

use crate::{
    collection::{COLLECTION_SEARCH_KEY, grid::CollectionGrid},
    common::storage::*,
    components::{
        modal::{MODAL_STACK, Modal, ModalBox},
        search::SearchBar,
    },
};
use api::{collection::*, search::SearchFilter};

#[component]
pub fn CollectionSearch() -> Element {
    let update_signal = use_signal(|| ());

    let collection_search_signal =
        use_signal::<String>(|| try_local_storage(COLLECTION_SEARCH_KEY));

    let collection_future = use_resource(move || async move {
        update_signal();

        let filter = collection_search_signal()
            .split_whitespace()
            .map(|s| s.to_owned())
            .collect();

        search_collections(&SearchCollectionsReq {
            filter: SearchFilter::SubstringAny { filter },
        })
        .await
    });

    let action_button = rsx! {
        div { style: "margin-left: auto;",
            button {
                class: "btn btn-primary",
                onclick: move |_| {
                    MODAL_STACK.with_mut(|v| v.push(Modal::CreateCollection));
                },
                "Create Collection"
            }
        }
    };

    let status = match &*collection_future.read() {
        Some(Ok(resp)) => format!("Found {} collections", resp.collections.len()),
        Some(Err(_)) => String::from("Error searching collections"),
        None => String::from("Loading..."),
    };

    rsx! {
        div { class: "container with-sticky",
            ModalBox { update_signal }

            div { class: "sticky-header",
                div {
                    class: "page-header",
                    style: "margin-bottom: var(--space-4);",
                    p { "Organize and browse your media collections" }
                }

                SearchBar {
                    search_signal: collection_search_signal,
                    storage_key: COLLECTION_SEARCH_KEY,
                    placeholder: "Search by collection name or description...",
                    status,
                    action_button,
                }
            }

            div { class: "scrollable-content",
                match &*collection_future.read() {
                    Some(Ok(resp)) => {
                        rsx! {
                            CollectionGrid { collections: resp.collections.clone() }
                        }
                    }
                    Some(Err(err)) => rsx! {
                        div {
                            class: "error-state",
                            style: "padding: var(--space-4); background-color: var(--surface); border-radius: var(--radius-lg); margin-top: var(--space-4); color: var(--error); text-align: center;",
                            "Error: {err}"
                        }
                    },
                    None => rsx! {
                        div {
                            class: "loading-state collections-grid",
                            style: "display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: var(--space-4); margin-top: var(--space-4);",
                            for _ in 0..6 {
                                div {
                                    class: "collection-card loading",
                                    style: "background-color: var(--surface); border-radius: var(--radius-lg); overflow: hidden; box-shadow: var(--shadow-sm); height: 100%;",
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
}
