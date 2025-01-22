use dioxus::prelude::*;

use crate::{
    common::{storage::*, style},
    gallery::{grid::MediaGrid, MEDIA_SEARCH_KEY},
};
use common::api::media::*;

// GalleryList elements
//
// this group of elements allows users to search all media that they
// can access by calling search_media(), and displays the results in
// a MediaGrid
//
// clicking on a grid tile jumps to GalleryDetail
#[derive(Clone, PartialEq, Props)]
struct GallerySearchBarProps {
    media_search_signal: Signal<String>,
    status: String,
}

#[component]
fn GallerySearchBar(props: GallerySearchBarProps) -> Element {
    let mut media_search_signal = props.media_search_signal;
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
                span { "MISSING: needs attention checkbox" }
            }
        }
    }
}

#[component]
pub fn GallerySearch() -> Element {
    let media_search_signal = use_signal::<String>(|| try_local_storage(MEDIA_SEARCH_KEY));

    let media_future = use_resource(move || async move {
        let filter = media_search_signal();

        search_media(&SearchMediaReq { filter: filter }).await
    });

    let (media, status) = match &*media_future.read() {
        Some(Ok(resp)) => (
            Ok(resp.media.clone()),
            format!("Found {} results", resp.media.len()),
        ),
        Some(Err(err)) => (
            Err(err.to_string()),
            String::from("Error from search_media"),
        ),
        None => (
            Err(String::from("Still waiting on search_media future...")),
            String::from(""),
        ),
    };

    rsx! {
        GallerySearchBar { media_search_signal, status }

        match media {
            Ok(media) => rsx! {
                MediaGrid { media: media }
            },
            Err(err) => rsx! {
                span { "{err}" }
            },
        }
    }
}
