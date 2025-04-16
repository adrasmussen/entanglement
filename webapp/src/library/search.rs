// webapp/src/library/search.rs

use dioxus::prelude::*;

use crate::{
    common::storage::*,
    components::{modal::ModalBox, search_bar::SearchBar},
    library::{table::LibraryTable, LIBRARY_SEARCH_KEY},
};
use api::library::*;

#[component]
pub fn LibrarySearch() -> Element {
    let update_signal = use_signal(|| ());

    let library_search_signal = use_signal::<String>(|| try_local_storage(LIBRARY_SEARCH_KEY));

    let library_future = use_resource(move || async move {
        update_signal.read();
        let filter = library_search_signal();
        search_libraries(&SearchLibrariesReq { filter }).await
    });

    let status = match &*library_future.read() {
        Some(Ok(resp)) => format!("Found {} libraries", resp.libraries.len()),
        Some(Err(_)) => String::from("Error searching libraries"),
        None => String::from("Loading..."),
    };

    rsx! {
        div { class: "container",
            ModalBox { update_signal }

            div { class: "page-header", style: "margin-bottom: var(--space-4);",
                h1 { class: "section-title", "Libraries" }
                p { "Manage your media source libraries" }
            }

            SearchBar {
                search_signal: library_search_signal,
                storage_key: LIBRARY_SEARCH_KEY,
                placeholder: "Search by library path...",
                status,
            }

            match &*library_future.read() {
                Some(Ok(resp)) => {
                    rsx! {
                        LibraryTable { libraries: resp.libraries.clone(), update_signal }
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
                        class: "loading-state libraries-table",
                        style: "
                                                margin-top: var(--space-4);
                                                background-color: var(--surface);
                                                border-radius: var(--radius-lg);
                                                overflow: hidden;
                                                box-shadow: var(--shadow-sm);
                                            ",
                        // Library table skeleton loading UI
                        table { style: "width: 100%; border-collapse: collapse;",
                            thead {
                                tr {
                                    for _ in 0..4 {
                                        th {
                                            div {
                                                class: "skeleton",
                                                style: "height: 24px; width: 100%;",
                                            }
                                        }
                                    }
                                }
                            }
                            tbody {
                                for _ in 0..5 {
                                    tr {
                                        for _ in 0..4 {
                                            td {
                                                div {
                                                    class: "skeleton",
                                                    style: "height: 18px; width: 90%;",
                                                }
                                            }
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
