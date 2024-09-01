use dioxus::prelude::*;

use serde::{Deserialize, Serialize};

use crate::common::{
    modal::{Modal, ModalBox},
    storage::*,
    style,
};
use crate::gallery::grid::MediaGrid;
use api::library::*;

mod list;
use list::LibraryList;

const LIBRARY_SEARCH_KEY: &str = "library_search";
const MEDIA_SEARCH_KEY: &str = "media_in_library_search";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
struct StoredLibraryMediaSearch {
    filter: String,
    hidden: bool,
}

// states of the library page
#[derive(Clone)]
enum LibraryView {
    LibraryList,
    MediaList(LibraryUuid),
}

#[derive(Clone, PartialEq, Props)]
struct LibraryNavBarProps {
    library_view_signal: Signal<LibraryView>,
    library_search_signal: Signal<String>,
    media_search_signal: Signal<StoredLibraryMediaSearch>,
    status: (String, String),
}

#[component]
fn LibraryNavBar(props: LibraryNavBarProps) -> Element {
    let mut library_view_signal = props.library_view_signal;
    let mut library_search_signal = props.library_search_signal;
    let mut media_search_signal = props.media_search_signal;

    let (library_status, media_status) = props.status;

    // somewhat unfortunate hack because dioxus requires us to always call the hook,
    // even if we don't end up using the output
    //
    // TODO -- better logging for the Error
    let library = use_resource(move || async move {
        let library_uuid = match library_view_signal() {
            LibraryView::MediaList(val) => val,
            LibraryView::LibraryList => return None,
        };

        match get_library(&GetLibraryReq {
            library_uuid: library_uuid,
        })
        .await
        {
            Ok(resp) => return Some(resp.library),
            Err(_) => return None,
        }
    });

    rsx! {
        div {
            style { "{style::SUBNAV}" }
            div {
                class: "subnav",
                match library_view_signal() {
                    LibraryView::LibraryList => rsx! {
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

                            },
                            input {
                                r#type: "submit",
                                value: "Search",
                            },
                        },
                        span { "Search History" },
                        span { "{library_status}"}
                        button { "Create Library" },
                    },
                    LibraryView::MediaList(_) => {
                        let library = &*library.read();

                        let library_path = match library.clone().flatten() {
                            Some(val) => val.path,
                            None => String::from("still waiting on get_library future...")
                        };

                        rsx! {
                            form {
                                onsubmit: move |event| async move {
                                    let filter = match event.values().get("media_search_filter") {
                                        Some(val) => val.as_value(),
                                        None => String::from(""),
                                    };

                                    let hidden = match event.values().get("hideen") {
                                        Some(val) => match val.as_value().as_str() {
                                            "true" => true,
                                            _ => false,
                                        },
                                        None => false,
                                    };

                                    let search = StoredLibraryMediaSearch {
                                        filter: filter,
                                        hidden: hidden,
                                    };

                                    media_search_signal.set(search.clone());

                                    set_local_storage(MEDIA_SEARCH_KEY, search);
                                },
                                input {
                                    name: "media_search_filter",
                                    r#type: "text",
                                    value: "{media_search_signal().filter}",

                                },
                                label {
                                    r#for: "hidden",
                                    "Hidden"
                                },
                                input {
                                    id: "hidden",
                                    name: "hidden",
                                    r#type: "checkbox",
                                    checked: media_search_signal().hidden,
                                    value: "true",
                                },
                                input {
                                    r#type: "submit",
                                    value: "Search",
                                },
                                span { "Search History" }
                                span { "Searching {library_path}: {media_status}" }
                                button { "View Library" },
                                button {
                                    onclick: move |_| library_view_signal.set(LibraryView::LibraryList),
                                    "Reset library search"
                                },
                            },
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn Libraries() -> Element {
    let modal_stack_signal = use_signal::<Vec<Modal>>(|| Vec::new());
    let library_view_signal = use_signal(|| LibraryView::LibraryList);

    // library search logic
    let library_search_signal = use_signal::<String>(|| try_local_storage(LIBRARY_SEARCH_KEY));
    let library_future = use_resource(move || async move {
        let filter = library_search_signal();

        search_libraries(&SearchLibrariesReq { filter: filter }).await
    });

    let (libraries, library_status) = match &*library_future.read() {
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

    // media search logic
    let media_search_signal = use_signal::<StoredLibraryMediaSearch>(|| try_local_storage(MEDIA_SEARCH_KEY));
    let media_future = use_resource(move || async move {
        let library_uuid = match library_view_signal() {
            LibraryView::MediaList(library_uuid) => library_uuid,
            LibraryView::LibraryList => {
                return Err(anyhow::Error::msg(
                    "library_uuid not specified in media_future",
                ))
            }
        };

        let search = media_search_signal();

        search_media_in_library(&SearchMediaInLibraryReq {
            library_uuid: library_uuid,
            filter: search.filter,
            hidden: search.hidden,
        })
        .await
    });

    let (media, media_status) = match &*media_future.read() {
        Some(Ok(resp)) => (
            Ok(resp.media.clone()),
            format!("Found {} results", resp.media.len()),
        ),
        Some(Err(err)) => (
            Err(err.to_string()),
            String::from("Error from search_media_in_library"),
        ),
        None => (
            Err(String::from(
                "Still waiting on search_media_in_library future...",
            )),
            String::from(""),
        ),
    };

    rsx! {
        LibraryNavBar {
            library_view_signal: library_view_signal,
            library_search_signal: library_search_signal,
            media_search_signal: media_search_signal,
            status: (library_status, media_status),
        }
        ModalBox { stack_signal: modal_stack_signal }

        match library_view_signal() {
            LibraryView::LibraryList => match libraries {
                Ok(libraries) => rsx! {
                    LibraryList { library_view_signal: library_view_signal, libraries: libraries }
                },
                Err(err) => rsx! {
                    span { "{err}" }
                },
            },
            LibraryView::MediaList(_) => match media {
                Ok(media) => rsx! {
                    MediaGrid { modal_stack_signal: modal_stack_signal, media: media}
                },
                Err(err) => rsx! {
                    span { "{err}" }
                },
            },
        }
    }
}
