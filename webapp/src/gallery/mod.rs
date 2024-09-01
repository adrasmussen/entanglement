use dioxus::prelude::*;

use crate::common::{
    modal::{Modal, ModalBox},
    storage::*,
    style,
};
use api::media::*;

pub mod grid;
use grid::MediaGrid;

const MEDIA_SEARCH_KEY: &str = "media_search";

#[derive(Clone, PartialEq, Props)]
struct GalleryNavBarProps {
    media_search_signal: Signal<String>,
    status: String,
}

#[component]
fn GalleryNavBar(props: GalleryNavBarProps) -> Element {
    let mut media_search_signal = props.media_search_signal;
    let status = props.status;

    rsx! {
        div {
            style { "{style::SUBNAV}" }
            div {
                class: "subnav",
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
                        value: "{media_search_signal()}",

                    },
                    input {
                        r#type: "submit",
                        value: "Search",
                    },
                },
                span { "Search History" },
                span { "{status}" },
            }
        }
    }
}

#[component]
pub fn Gallery() -> Element {
    let modal_stack_signal = use_signal::<Vec<Modal>>(|| Vec::new());
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
        GalleryNavBar { media_search_signal: media_search_signal, status: status }
        ModalBox { stack_signal: modal_stack_signal }

        match media {
            Ok(media) => rsx! {
                MediaGrid { modal_stack_signal: modal_stack_signal, media: media }
            },
            Err(err) => rsx! {
                span { "{err}" }
            },
        }
    }
}
