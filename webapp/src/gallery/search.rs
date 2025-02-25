use dioxus::prelude::*;
use tracing::debug;

use crate::{
    common::{storage::*, style},
    gallery::{grid::MediaGrid, MEDIA_SEARCH_KEY},
};
use api::media::*;

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
                        debug!("updating media_search_signal");
                        media_search_signal.set(filter.clone());
                        set_local_storage(MEDIA_SEARCH_KEY, filter);
                    },
                    input {
                        name: "search_filter",
                        r#type: "text",
                        value: "{media_search_signal}",
                    }
                    input { r#type: "submit", value: "Search" }
                }
                span { "{status}" }
                span { "TODO: bulk select bar" }
            }
        }
    }
}

//
// ROUTE TARGET
//
#[component]
pub fn GallerySearch() -> Element {
    // this signal must only be set in an onclick or similar, and never by the component
    // functions lest we trigger an infinite loop
    let media_search_signal = use_signal::<String>(|| try_local_storage(MEDIA_SEARCH_KEY));

    // use_resource() automatically subscribes to updates from hooks that it reads, so setting
    // a new search signal re-runs the future...
    let media_future = use_resource(move || async move {
        let filter = media_search_signal;

        search_media(&SearchMediaReq { filter: filter() }).await
    });

    // ... which in turn re-renders the whole component because it is subscribed to that hook
    match &*media_future.read_unchecked() {
        Some(Ok(resp)) => {
            return rsx! {
                GallerySearchBar {
                    media_search_signal,
                    status: format!("Found {} results", resp.media.len()),
                }
                // this clone is not great -- in principle, there may be quite a lot of stuff
                // in the vec, especially at this call site
                MediaGrid { media: resp.media.clone() }
            };
        }
        Some(Err(err)) => {
            return rsx! {
                GallerySearchBar {
                    media_search_signal,
                    status: String::from("Error from search_media"),
                }
                span { "{err}" }
            }
        }
        None => {
            return rsx! {
                span { "loading..." }
            }
        }
    }
}
