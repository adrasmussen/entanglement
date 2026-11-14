use dioxus::prelude::*;

use crate::{
    common::search::parse_query,
    components::{
        modal::ModalBox,
        query_bar::{QueryBar, use_search_state},
    },
    library::{LIBRARY_SEARCH_KEY, table::LibraryTable},
};
use api::library::*;

#[component]
pub fn LibrarySearch() -> Element {
    let update_signal = use_signal(|| ());

    let search = use_search_state(LIBRARY_SEARCH_KEY, "", use_callback(|_: String| {}));
    let library_search_signal = search.signal;

    let library_future = use_resource(move || async move {
        update_signal();

        let search = library_search_signal();
        search_libraries(&SearchLibrariesReq {
            query: Some(parse_query::<Library>(&search)),
            ..Default::default()
        })
        .await
    });

    let status = match &*library_future.read() {
        Some(Ok(resp)) => format!("Found {} libraries", resp.libraries.len()),
        Some(Err(_)) => String::from("Error searching libraries"),
        None => String::from("Loading..."),
    };

    rsx! {
            div { class: "container with-sticky",
                ModalBox { update_signal }

                div { class: "sticky-header",
                    div {
                        class: "page-header",
                        style: "margin-bottom: var(--space-4);",
                        p { "Manage your media source libraries" }
                    }

                    QueryBar::<Library> {
     search,
    status,
                    }
                }

                div { class: "scrollable-content",
                    match &*library_future.read() {
                        Some(Ok(resp)) => {
                            rsx! {
                                LibraryTable { libraries: resp.libraries.clone() }
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
                                class: "loading-state libraries-table",
                                style: "margin-top: var(--space-4);  background-color: var(--surface); border-radius: var(--radius-lg); overflow: hidden; box-shadow: var(--shadow-sm);",
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
}
