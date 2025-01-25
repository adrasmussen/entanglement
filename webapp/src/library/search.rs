use dioxus::prelude::*;

use crate::{
    common::{storage::*, style},
    library::{table::LibraryTable, LIBRARY_SEARCH_KEY},
};
use api::library::*;

#[derive(Clone, PartialEq, Props)]
struct LibrarySearchBarProps {
    library_search_signal: Signal<String>,
    status: String,
}

#[component]
fn LibrarySearchBar(props: LibrarySearchBarProps) -> Element {
    let mut library_search_signal = props.library_search_signal;
    let status = props.status;

    rsx! {
        div {
            style { "{style::SUBNAV}" }
            div { class: "subnav",
                form {
                    onsubmit: move |event| async move {
                        let filter = match event.values().get("search_filter") {
                            Some(val) => val.as_value(),
                            None => String::from(""),
                        };
                        library_search_signal.set(filter.clone());
                        set_local_storage(LIBRARY_SEARCH_KEY, filter);
                    },
                    input {
                        name: "search_filter",
                        r#type: "text",
                        value: "{library_search_signal()}",
                    }
                    input { r#type: "submit", value: "Search" }
                }
                span { "Search History" }
                span { "{status}" }
                span { "MISSING: create library modal" }
            }
        }
    }
}

#[component]
pub fn LibrarySearch() -> Element {
    let library_search_signal = use_signal::<String>(|| try_local_storage(LIBRARY_SEARCH_KEY));

    let library_future = use_resource(move || async move {
        let filter = library_search_signal();

        search_libraries(&SearchLibrariesReq { filter: filter }).await
    });

    let (libraries, status) = match &*library_future.read() {
        Some(Ok(resp)) => (
            Ok(resp.libraries.clone()),
            format!("Found {} results", resp.libraries.len()),
        ),
        Some(Err(err)) => (
            Err(err.to_string()),
            String::from("Error from search_libraries"),
        ),
        None => (
            Err(String::from("Still waiting on search_libraries future...")),
            String::from(""),
        ),
    };

    rsx! {
        LibrarySearchBar { library_search_signal, status }

        match libraries {
            Ok(libraries) => rsx! {
                LibraryTable { libraries }
            },
            Err(err) => rsx! {
                span { "{err}" }
            },
        }
    }
}
