use dioxus::prelude::*;

use crate::{
    common::storage::try_local_storage,
    components::{
        advanced::AdvancedContainer, media_card::MediaCard, modal::ModalBox, search_bar::SearchBar,
    },
    gallery::MEDIA_SEARCH_KEY,
};
use api::{media::*, search::SearchFilter};

#[component]
pub fn GallerySearch() -> Element {
    let update_signal = use_signal(|| ());

    let mut advanced_expanded = use_signal(|| false);

    let media_search_signal = use_signal::<String>(|| try_local_storage(MEDIA_SEARCH_KEY));

    let media_future = use_resource(move || async move {
        let filter = media_search_signal()
            .split_whitespace()
            .map(|s| s.to_owned())
            .collect();

        search_media(&SearchMediaReq {
            filter: SearchFilter::SubstringAny { filter },
        })
        .await
    });

    let action_button = rsx! {
        button {
            class: "btn btn-secondary",
            onclick: move |_| {
                advanced_expanded.set(!advanced_expanded());
            },
            if advanced_expanded() {
                "Hide Advanced"
            } else {
                "Advanced"
            }
        }
    };

    rsx! {
        div { class: "container with-sticky",
            ModalBox { update_signal }

            div { class: "sticky-header",
                div {
                    class: "page-header",
                    style: "margin-bottom: var(--space-4);",
                    h1 { class: "section-title", "Photo Gallery" }
                    p { "Browse and search all accessible media" }
                }

                SearchBar {
                    search_signal: media_search_signal,
                    storage_key: MEDIA_SEARCH_KEY,
                    placeholder: "Search by date or description...",
                    status: match &*media_future.read() {
                        Some(Ok(resp)) => format!("Found {} results", resp.media.len()),
                        Some(Err(_)) => String::from("Error searching media"),
                        None => String::from("Loading..."),
                    },
                    action_button,
                }

                {
                    if advanced_expanded() {
                        rsx! {
                            AdvancedContainer { media_search_signal }
                        }
                    } else {
                        rsx! {}
                    }
                }
            }

            div { class: "scrollable-content",
                match &*media_future.read() {
                    Some(Ok(resp)) => {
                        rsx! {
                            if resp.media.is_empty() {
                                div { class: "empty-state",
                                    p { "No media found matching your search criteria." }
                                }
                            } else {
                                div { class: "media-grid",
                                    for media_uuid in resp.media.iter() {
                                        MediaCard { key: "{media_uuid}", media_uuid: *media_uuid }
                                    }
                                }
                            }
                        }
                    }
                    Some(Err(err)) => rsx! {
                        div { class: "error-state",
                            p { "Error: {err}" }
                        }
                    },
                    None => rsx! {
                        div { class: "loading-state media-grid",
                            for _ in 0..8 {
                                div { class: "skeleton-card",
                                    div { class: "skeleton", style: "height: 200px;" }
                                    div {
                                        class: "skeleton",
                                        style: "height: 24px; width: 40%; margin-top: 12px;",
                                    }
                                    div {
                                        class: "skeleton",
                                        style: "height: 18px; width: 80%; margin-top: 8px;",
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
