use dioxus::prelude::*;

use crate::{
    library::{grid::MediaGrid, MEDIA_SEARCH_KEY},
    common::{storage::*, style},
};
use api::library::*;

#[derive(Clone, PartialEq, Props)]
struct LibraryDetailBarProps {
    media_search_signal: Signal<String>,
    library_uuid: LibraryUuid,
    status: String,
}

#[component]
fn LibraryDetailBar(props: LibraryDetailBarProps) -> Element {
    let mut media_search_signal = props.media_search_signal;
    let library_uuid = props.library_uuid;
    let status = props.status;

    let library_future = use_resource(move || async move {
        get_library(&GetLibraryReq {
            library_uuid: library_uuid,
        })
        .await
    });

    let library = &*library_future.read();

    let library_result = match library {
        Some(Ok(result)) => result.library.clone(),
        _ => {
            return rsx! {
                div { class: "subnav",
                    span { "error fetching {library_uuid}" }
                }
            }
        }
    };

    rsx! {
        div {
            style { "{style::SUBNAV}" }
            div { class: "subnav",
                span { "library: {library_result.path}" }
                span { "Group: {library_result.gid}" }
                span { "MISSING: HIDDEN CHECKBOX" }
            }
        }
        div {
            style { "{style::SUBNAV}" }
            div { class: "subnav",
                form {
                    onsubmit: move |event| async move {
                        let filter = match event.values().get("search_filter") {
                            Some(val) => val.as_value(),
                            None => String::from(""),
                        };
                        media_search_signal.set(filter.clone());
                        set_local_storage(MEDIA_SEARCH_KEY, filter);
                    },
                    input {
                        name: "search_filter",
                        r#type: "text",
                        value: "{media_search_signal()}"
                    }
                    input { r#type: "submit", value: "Search" }
                }
                span { "Search History" }
                span { "{status}" }
            }
        }
    }
}


#[derive(Clone, PartialEq, Props)]
pub struct LibraryDetailProps {
    // this is a String because we get it from the Router
    library_uuid: String,
}

#[component]
pub fn LibraryDetail(props: LibraryDetailProps) -> Element {
    let library_uuid = match props.library_uuid.parse::<LibraryUuid>() {
        Ok(uuid) => uuid,
        Err(_) => {
            return rsx! {
                span { "failed to convert library_uuid" }
            }
        }
    };

    let media_search_signal = use_signal::<String>(|| try_local_storage(MEDIA_SEARCH_KEY));

    // TODO -- missing hidden signal
    let media_future = use_resource(move || async move {
        let filter = media_search_signal();

        search_media_in_library(&SearchMediaInLibraryReq {
            library_uuid: library_uuid,
            filter: filter,
            hidden: false,
        })
        .await
    });

    let (media, status) = match &*media_future.read() {
        Some(Ok(resp)) => (
            Ok(resp.media.clone()),
            format!("Found {} results", resp.media.len()),
        ),
        Some(Err(err)) => (
            Err(err.to_string()),
            String::from("Error from search_media_in_library"),
        ),
        None => (
            Err(String::from("Still waiting on search_media_in_library
             future...")),
            String::from(""),
        ),
    };

    rsx!(
        LibraryDetailBar { media_search_signal, library_uuid, status }

        match media {
            Ok(media) => rsx! {
                MediaGrid {media: media }
            },
            Err(err) => rsx! {
                span { "{err}" }
            },
        }
    )
}
