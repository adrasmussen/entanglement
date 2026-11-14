use dioxus::prelude::*;

use crate::{
    collection::{COLLECTION_SEARCH_KEY, grid::CollectionGrid},
    common::{
        paging::{Page, use_paged},
        search::parse_query,
        storage::*,
    },
    components::{
        modal::{MODAL_STACK, Modal, ModalBox},
        search::SearchBar,
    },
};
use api::{collection::*, page::PageRequest};

#[component]
pub fn CollectionSearch() -> Element {
    let update_signal = use_signal(|| ());

    let collection_search_signal =
        use_signal::<String>(|| try_local_storage(COLLECTION_SEARCH_KEY));

    let paged = use_paged(
        move || collection_search_signal.cloned(),
        update_signal,
        // cards here load their own data, so heights are not stable on a first frame
        None,
        |search: String, offset, limit, with_total| async move {
            let resp = search_collections(&SearchCollectionsReq {
                filter: String::new(),
                query: Some(parse_query::<Collection>(&search)),
                page: PageRequest {
                    limit: Some(limit),
                    offset,
                    with_total,
                    ..Default::default()
                },
            })
            .await?;

            // uuid-only, so nothing goes missing between the search and the response
            Ok(Page::from_resp(resp))
        },
    );
    let pager = paged.pager;

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

    let status = pager.status(paged.items.read().len(), "collections");

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
                    placeholder: "Search by collection name...",
                    status,
                    action_button,
                }
            }

            div { class: "scrollable-content",
                if !(pager.initialized)() {
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
                } else if let (Some(err), true) = ((pager.error)(), paged.items.read().is_empty()) {
                    div {
                        class: "error-state",
                        style: "padding: var(--space-4); background-color: var(--surface); border-radius: var(--radius-lg); margin-top: var(--space-4); color: var(--error); text-align: center;",
                        "Error: {err}"
                        div {
                            button {
                                class: "btn btn-secondary",
                                style: "margin-top: var(--space-2);",
                                onclick: move |_| pager.load_more.call(()),
                                "Retry"
                            }
                        }
                    }
                } else {
                    CollectionGrid { collections: paged.items, pager }
                }
            }
        }
    }
}
