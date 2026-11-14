use std::collections::HashMap;

use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::{
    Route,
    collection::MEDIA_SEARCH_KEY,
    common::{
        grid_cache::COLLECTION_META,
        paging::{CacheCfg, use_paged},
        query_ui::q_param,
        search::{GridScope, fetch_media_page, reconcile_media, search_opts},
    },
    components::{
        advanced::{AdvancedSearchTab, BulkEditMode, BulkEditTab, CollectionColorTab},
        media_grid::{GridDensityMenu, MediaGrid},
        modal::{MODAL_STACK, Modal, ModalBox},
        query_bar::{QueryBar, use_search_state},
        sidebar::AdvancedSidebar,
    },
};
use api::{
    UuidSource,
    collection::*,
    fold_set,
    media::Media,
    search::{SearchRequest, SearchResponse},
};

#[derive(Clone, PartialEq, Props)]
pub struct CollectionDetailProps {
    // This is a String because we get it from the Router
    collection_uuid: String,
    // the escaped search text from ?q=, see q_param
    q: String,
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
            CollectionInner {
                update_signal,
                collection_uuid: props.collection_uuid,
                q: props.q,
            }
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
            Link { to: Route::CollectionSearch { q: String::new() }, class: "btn btn-primary", "Return to Collections" }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
struct CollectionInnerProps {
    update_signal: Signal<()>,
    collection_uuid: String,
    q: String,
}

impl UuidSource for CollectionInnerProps {}

#[component]
fn CollectionInner(props: CollectionInnerProps) -> Element {
    let update_signal = props.update_signal;

    let collection_uuid = CollectionUuid::try_parse(&props, &props.collection_uuid).show(|_| {
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
        update_signal();
        let collection_uuid = collection_uuid();

        let resp = get_collection(&GetCollectionReq { collection_uuid }).await;
        if let Ok(resp) = &resp {
            COLLECTION_META
                .write()
                .insert(collection_uuid, resp.clone());
        }
        resp
    });

    let nav = use_navigator();
    let on_commit = use_callback(move |text: String| {
        let _ = nav.replace(Route::CollectionDetail {
            collection_uuid: collection_uuid.peek().to_string(),
            q: q_param(&text),
        });
    });
    let search = use_search_state(MEDIA_SEARCH_KEY, &props.q, on_commit);
    let media_search_signal = search.signal;
    let mut advanced_expanded = use_signal(|| false);
    let mut bulk_edit_signal = use_signal(|| None);
    let mut collection_color_signal = use_signal(HashMap::new);

    // all hooks must run before the early returns below
    let paged = use_paged(
        move || (collection_uuid(), media_search_signal()),
        update_signal,
        Some(CacheCfg::new(
            "collection",
            |(collection_uuid, search): (CollectionUuid, String), old: SearchResponse| async move {
                reconcile_media(GridScope::Collection(collection_uuid), &search, old).await
            },
        )),
        |(collection_uuid, search): (CollectionUuid, String), offset, limit, with_total| async move {
            fetch_media_page(
                SearchRequest::Collection(SearchMediaInCollectionReq {
                    collection_uuid,
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
        SearchRequest::Collection(SearchMediaInCollectionReq {
            collection_uuid: *collection_uuid.peek(),
            opts: search_opts(&media_search_signal.peek()),
        })
    });

    // see GalleryInner for details
    //
    // only the collection metadata gates the page skeleton; media paging handles its own loading state
    let collection_data = &*collection_future.read();
    let collection_data = match collection_data.clone().transpose().show(|error| {
        rsx! {
            CollectionError { message: format!("There was an error fetching the collection metadata: {error}") }
        }
    })? {
        // remembered from the last visit, so that coming back does not flash a skeleton (and
        // shrink the page under the restored scroll position); the resource refreshes it
        None => match COLLECTION_META.peek().get(&collection_uuid()).cloned() {
            Some(v) => v,
            None => {
                return rsx! {
                    CollectionDetailSkeleton {}
                };
            }
        },
        Some(v) => v,
    };

    let collection = collection_data.collection;

    let formatted_tags = fold_set(collection.tags.clone())
        .unwrap_or_else(|_| "invalid tags, contact admins".to_string());

    let action_button = rsx! {
        div { style: "display: flex; align-items: center; gap: var(--space-2); margin-left: auto;",
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
                        class: "collection-detail-header",
                        style: "background-color: var(--surface); border-radius: var(--radius-lg); padding: var(--space-4); margin-bottom: var(--space-4); box-shadow: var(--shadow-sm);",
                        div { style: "display: flex; justify-content: space-between; align-items: flex-start;",
                            div {
                                h1 { style: "margin: 0 0 var(--space-2) 0;", "{collection.name}" }
                                div { style: "display: flex; gap: var(--space-4); margin-bottom: var(--space-3); color: var(--text-secondary); font-size: 0.875rem;",
                                    span { "Owner: {collection.uid}" }
                                    span { "Group: {collection.gid}" }
                                }

                                if !collection.note.is_empty() {
                                    p { style: "padding: var(--space-3); background-color: var(--neutral-50); border-radius: var(--radius-md); font-style: italic; color: var(--text-secondary); max-width: 700px;",
                                        "{collection.note}"
                                    }
                                }
                                if !collection.tags.is_empty() {
                                    p { style: "padding: var(--space-3); background-color: var(--neutral-50); border-radius: var(--radius-md); font-style: italic; color: var(--text-secondary); max-width: 700px;",
                                        "Tags: {formatted_tags}"
                                    }
                                }
                            }

                            div { style: "display: flex; gap: var(--space-2); align-items: center;",
                                GridDensityMenu {}
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

                    QueryBar::<Media> {
     search,
    status: paged.pager.status(paged.items.read().len(), "results"),
                        action_button,
                    }

                    AdvancedSidebar {
                        show_signal: advanced_expanded,
                        tabs: HashMap::from([
                            ("Advanced Search".to_owned(), rsx! {
                                AdvancedSearchTab::<Media> { search }
                            }),
                            ("Bulk Edit".to_owned(), rsx! {
                                BulkEditTab {
                                    bulk_edit_signal,
                                    search_req,
                                    modes: Vec::from([BulkEditMode::EditTags, BulkEditMode::RmFromCollection(collection_uuid())]),
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
                        collection_uuid: collection_uuid(),
                        empty: rsx! {
                            div {
                                class: "empty-state",
                                style: "padding: var(--space-8) var(--space-4); text-align: center; background-color: var(--surface); border-radius: var(--radius-lg); margin-top: var(--space-4);",
                                div { style: "font-size: 4rem; margin-bottom: var(--space-4); color: var(--neutral-400);",
                                    "🖼️"
                                }
                                h3 { style: "margin-bottom: var(--space-2); color: var(--text-primary);",
                                    "No Media in This Collection"
                                }
                                p { style: "color: var(--text-secondary); max-width: 500px; margin: 0 auto;",
                                    "This collection doesn't contain any media yet. Add some media to get started."
                                }
                            }
                        },
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
