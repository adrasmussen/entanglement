use std::collections::HashMap;

use dioxus::prelude::*;

use crate::{
    common::{
        paging::{CacheCfg, use_paged},
        search::{GridScope, fetch_media_page, reconcile_media, search_opts},
        storage::try_local_storage,
    },
    components::{
        advanced::{AdvancedSearchTab, BulkEditMode, BulkEditTab, CollectionColorTab},
        media_grid::{GridDensityMenu, MediaGrid},
        modal::ModalBox,
        search::SearchBar,
        sidebar::AdvancedSidebar,
    },
    gallery::MEDIA_SEARCH_KEY,
};
use api::{
    media::SearchMediaReq,
    search::{SearchRequest, SearchResponse},
};

#[component]
pub fn GallerySearch() -> Element {
    let update_signal = use_signal(|| ());

    let media_search_signal = use_signal::<String>(|| try_local_storage(MEDIA_SEARCH_KEY));
    let mut advanced_expanded = use_signal(|| false);
    let mut bulk_edit_signal = use_signal(|| None);
    let mut collection_color_signal = use_signal(HashMap::new);

    let paged = use_paged(
        move || media_search_signal.cloned(),
        update_signal,
        Some(CacheCfg::new(
            "gallery",
            |search: String, old: SearchResponse| async move {
                reconcile_media(GridScope::Gallery, &search, old).await
            },
        )),
        |search: String, offset, limit, with_total| async move {
            fetch_media_page(
                SearchRequest::Media(SearchMediaReq {
                    opts: search_opts(&search),
                }),
                offset,
                limit,
                with_total,
            )
            .await
        },
    );

    let search_req = use_callback(move |_: ()| {
        SearchRequest::Media(SearchMediaReq {
            opts: search_opts(&media_search_signal.peek()),
        })
    });

    let action_button = rsx! {
        div { style: "display: flex; align-items: center; gap: var(--space-2);",
            button {
                class: "btn btn-secondary",
                onclick: move |_| {
                    if advanced_expanded() {
                        bulk_edit_signal.set(None);
                        collection_color_signal.set(HashMap::new());
                    }
                    advanced_expanded.set(!advanced_expanded());
                },

                if advanced_expanded() {
                    "Hide Advanced"
                } else {
                    "Advanced"
                }
            }
        }
    };

    rsx! {
        div { class: "container with-sticky",
            ModalBox { update_signal }

            div { class: "sticky-header",
                div {
                    class: "page-header",
                    style: "margin-bottom: var(--space-4); display: flex; justify-content: space-between; align-items: center; gap: var(--space-4);",
                    p { "Browse and search all accessible media" }
                    GridDensityMenu {}
                }

                SearchBar {
                    search_signal: media_search_signal,
                    storage_key: MEDIA_SEARCH_KEY,
                    placeholder: "Search by date, description or tags...",
                    status: paged.pager.status(paged.items.read().len(), "results"),
                    action_button,
                }

                AdvancedSidebar {
                    show_signal: advanced_expanded,
                    tabs: HashMap::from([
                        ("Advanced Search".to_owned(), rsx! {
                            AdvancedSearchTab { media_search_signal }
                        }),
                        ("Bulk Edit".to_owned(), rsx! {
                            BulkEditTab {
                                bulk_edit_signal,
                                search_req,
                                modes: Vec::from([BulkEditMode::EditTags, BulkEditMode::AddToCollection]),
                            }
                        }),
                        ("Collection Labels".to_owned(), rsx! {
                            CollectionColorTab { collection_color_signal }
                        }),
                    ]),
                }
            }

            div { class: "scrollable-content",
                MediaGrid {
                    items: paged.items,
                    window: paged.window,
                    pager: paged.pager,
                    bulk_edit_signal,
                    collection_color_signal,
                    empty: rsx! {
                        div { class: "empty-state",
                            p { "No media found matching your search criteria." }
                        }
                    },
                }
            }
        }
    }
}
