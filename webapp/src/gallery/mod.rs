use dioxus::prelude::*;

use crate::common::{media::MediaGrid, storage::*, style};
use api::media::*;

#[derive(Clone, PartialEq, Props)]
struct GalleryNavBarProps {
    search_filter_signal: Signal<SearchMediaReq>,
    search_status: String,
}

#[component]
fn GalleryNavBar(props: GalleryNavBarProps) -> Element {
    let mut search_filter_signal = props.search_filter_signal.clone();

    let search_filter = search_filter_signal().filter;

    rsx! {
        div {
            style { "{style::SUBNAV}" }
            div {
                class: "subnav",
                form {
                    onsubmit: move |event| {
                        let filter = match event.values().get("search_filter") {
                            Some(val) => val.as_value(),
                            None => String::from(""),
                        };
                        search_filter_signal.set(SearchMediaReq{filter: filter.clone()});

                        set_local_storage("gallery_search_filter", filter);
                    },
                    input {
                        name: "search_filter",
                        r#type: "text",
                        value: "{search_filter}",

                    },
                    input {
                        r#type: "submit",
                        value: "Search",
                    },
                },
                span { "Search History" },
                span { "{props.search_status}" },
            }
        }
    }
}

#[component]
pub fn Gallery() -> Element {
    let search_filter_signal = use_signal(|| SearchMediaReq {
        filter: match get_local_storage("gallery_search_filter") {
            Ok(val) => val,
            Err(_) => String::from(""),
        },
    });

    // call to the api server
    let search_results =
        use_resource(move || async move { search_media(&search_filter_signal()).await });

    // annoying hack to get around the way the signals are implemented
    let search_results = &*search_results.read();

    let (results, status) = match search_results {
        Some(Ok(results)) => (
            Ok(results.media.clone()),
            format!("found {} results", results.media.len()),
        ),
        Some(Err(err)) => (Err(err.to_string()), format!("error while searching")),
        None => (
            Ok(Vec::new()),
            format!("still awaiting search_media future..."),
        ),
    };

    rsx! {
        GalleryNavBar { search_filter_signal: search_filter_signal, search_status: status }
        MediaGrid { media: results }
    }
}
