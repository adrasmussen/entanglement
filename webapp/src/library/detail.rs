use std::collections::HashMap;

use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::{
    Route,
    common::{
        grid_cache::{LIBRARY_META, LIBRARY_SHOW_HIDDEN},
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
    library::{MEDIA_SEARCH_KEY, taskbar::TaskBar},
};
use api::{
    UuidSource,
    library::*,
    media::Media,
    search::{SearchRequest, SearchResponse},
};

#[derive(Clone, PartialEq, Props)]
pub struct LibraryDetailProps {
    // This is a String because we get it from the Router
    library_uuid: String,
    // the escaped search text from ?q=, see q_param
    q: String,
}

#[component]
pub fn LibraryDetail(props: LibraryDetailProps) -> Element {
    let update_signal = use_signal(|| ());

    rsx! {
        ModalBox { update_signal }
        ErrorBoundary {
            handle_error: |error: ErrorContext| {
                rsx! {
                    if let Some(error_ui) = error.show() {
                        {error_ui}
                    } else {
                        div { "LibraryDetail encountered an error.  Check the logs or reach out the the administrators." }
                    }
                }
            },
            LibraryInner {
                update_signal,
                library_uuid: props.library_uuid,
                q: props.q,
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct LibraryErrorProps {
    message: String,
}

#[component]
fn LibraryError(props: LibraryErrorProps) -> Element {
    rsx! {
        div { class: "container error-state",
            h1 { "Error Loading Library" }
            p { "There was an error loading the library: {props.message}" }
            Link { to: Route::LibrarySearch { q: String::new() }, class: "btn btn-primary", "Return to Libraries" }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
struct LibraryInnerProps {
    update_signal: Signal<()>,
    library_uuid: String,
    q: String,
}

impl UuidSource for LibraryInnerProps {}

#[component]
fn LibraryInner(props: LibraryInnerProps) -> Element {
    let update_signal = props.update_signal;

    let library_uuid = LibraryUuid::try_parse(&props, &props.library_uuid).show(|_| {
        let message = "The library_uuid could not be parsed".to_string();
        rsx! {
            LibraryError { message }
        }
    })?;

    // see GalleryInner for details
    let library_uuid = use_memo(use_reactive(&library_uuid, |library_uuid| library_uuid));
    let library_future = use_resource(move || async move {
        update_signal();

        let library_uuid = library_uuid();
        let resp = get_library(&GetLibraryReq { library_uuid }).await;
        if let Ok(resp) = &resp {
            LIBRARY_META.write().insert(library_uuid, resp.clone());
        }
        resp
    });

    // the library media search is the only place where we can specify hidden = true
    let mut show_hidden = use_signal(|| {
        LIBRARY_SHOW_HIDDEN
            .peek()
            .get(&*library_uuid.peek())
            .copied()
            .unwrap_or(false)
    });
    use_effect(move || {
        let v = show_hidden();
        LIBRARY_SHOW_HIDDEN.write().insert(*library_uuid.peek(), v);
    });
    let nav = use_navigator();
    let on_commit = use_callback(move |text: String| {
        let _ = nav.replace(Route::LibraryDetail {
            library_uuid: library_uuid.peek().to_string(),
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
        move || (library_uuid(), media_search_signal(), show_hidden()),
        update_signal,
        Some(CacheCfg::new(
            "library",
            |(_, search, hidden): (LibraryUuid, String, bool), old: SearchResponse| async move {
                reconcile_media(GridScope::Library { hidden }, &search, old).await
            },
        )),
        |(library_uuid, search, hidden): (LibraryUuid, String, bool), offset, limit, with_total| async move {
            fetch_media_page(
                SearchRequest::Library(SearchMediaInLibraryReq {
                    library_uuid,
                    hidden: Some(hidden),
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
        SearchRequest::Library(SearchMediaInLibraryReq {
            library_uuid: *library_uuid.peek(),
            hidden: Some(*show_hidden.peek()),
            opts: search_opts(&media_search_signal.peek()),
        })
    });

    // see GalleryInner for details
    //
    // only the library metadata gates the page skeleton; media paging handles its own loading state
    let library_data = &*library_future.read();
    let library_data = match library_data.clone().transpose().show(|error| {
        rsx! {
            LibraryError { message: format!("There was an error fetching the library metadata: {error}") }
        }
    })? {
        // see CollectionInner
        None => match LIBRARY_META.peek().get(&library_uuid()).cloned() {
            Some(v) => v,
            None => {
                return rsx! {
                    LibraryDetailSkeleton {}
                };
            }
        },
        Some(v) => v,
    };

    let library = library_data.library;

    // search bar action button
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

            input {
                r#type: "checkbox",
                id: "show-hidden-checkbox",
                checked: show_hidden(),
                oninput: move |evt| {
                    show_hidden.set(evt.checked());
                },
                style: "margin: 0 8px 0 8px;",
            }
            label { r#for: "show-hidden-checkbox", style: "margin: 0 16px 0 0;", "Show hidden files" }
        }
    };

    rsx! {
        // the ModalBox is in LibraryDetail, outside the error boundary; a second one here
        // drew every modal twice
        div { class: "container with-sticky",

            div { class: "sticky-header",
                // library detail view header
                div {
                    class: "library-detail-header",
                    style: "background-color: var(--surface); border-radius: var(--radius-lg); padding: var(--space-4); margin-bottom: var(--space-4); box-shadow: var(--shadow-sm);",
                    div { style: "display: flex; justify-content: space-between; align-items: flex-start;",
                        // Library info
                        div {
                            h1 { style: "margin: 0 0 var(--space-2) 0;", "Library: {library.path}" }
                            div { style: "display: flex; gap: var(--space-4); margin-bottom: var(--space-3); color: var(--text-secondary) font-size: 0.875rem;",
                                span { "Owner: {library.uid}" }
                                span { "Group: {library.gid}" }
                                //span { "Last scanned: {formatted_time}" }
                                span { "File count: {library.count}" }
                            }
                            TaskBar { update_signal, library_uuid }
                        }
                        // Action buttons
                        div { style: "display: flex; gap: var(--space-2); align-items: center;",
                            GridDensityMenu {}
                            button {
                                class: "btn btn-secondary",
                                onclick: move |_| {
                                    MODAL_STACK.with_mut(|v| v.push(Modal::StartTask(library_uuid())));
                                },
                                "Start Task"
                            }
                            button {
                                class: "btn btn-secondary",
                                onclick: move |_| {
                                    MODAL_STACK.with_mut(|v| v.push(Modal::TaskHistory(library_uuid())));
                                },
                                "Task History"
                            }
                        }
                    }
                }

                QueryBar::<Media> {
                    search,
                    status: paged.pager.status(paged.items.read().len(), if show_hidden() { "hidden results" } else { "results" }),
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
                        div {
                            class: "empty-state",
                            style: "padding: var(--space-8) var(--space-4); text-align: center; background-color: var(--surface); border-radius: var(--radius-lg); margin-top: var(--space-4);",
                            div { style: "font-size: 4rem; margin-bottom: var(--space-4); color: var(--neutral-400);",
                                "🖼️"
                            }
                            h3 { style: "margin-bottom: var(--space-2); color: var(--text-primary);",
                                "No Media Found"
                            }
                            p { style: "color: var(--text-secondary); max-width: 500px; margin: 0 auto;",
                                if show_hidden() {
                                    "No media matches your search criteria in this library."
                                } else {
                                    "No media matches your search criteria. Try different search terms or check the 'Show hidden files' option."
                                }
                            }
                            if !show_hidden() {
                                button {
                                    class: "btn btn-secondary",
                                    style: "margin-top: var(--space-4);",
                                    onclick: move |_| {
                                        show_hidden.set(true);
                                    },
                                    "Show Hidden Files"
                                }
                            }
                        }
                    },
                }
            }
        }
    }
}

#[component]
fn LibraryDetailSkeleton() -> Element {
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
            div { style: "display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: var(--space-4);",
                for _ in 0..6 {
                    div { class: "skeleton", style: "height: 200px;" }
                }
            }
        }
    }
}
