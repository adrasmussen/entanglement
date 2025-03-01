use dioxus::prelude::*;

use crate::{
    common::{
        modal::{Modal, ModalBox, MODAL_STACK},
        storage::try_local_storage,
    },
    components::{
        search_bar::SearchBar,
        media_card::MediaCard,
    },
    gallery::MEDIA_SEARCH_KEY,
};
use api::media::*;

#[component]
pub fn ModernGallerySearch() -> Element {
    let update_signal = use_signal(|| ());

    // Get search signal from local storage
    let media_search_signal = use_signal::<String>(|| try_local_storage(MEDIA_SEARCH_KEY));

    // Fetch media data
    let media_future = use_resource(move || async move {
        let filter = media_search_signal();
        search_media(&SearchMediaReq { filter }).await
    });

    // Create action button for search bar
    let action_button = rsx! {
        button {
            class: "btn btn-secondary",
            onclick: move |_| {
                MODAL_STACK.with_mut(|v| v.push(Modal::ShowAlbum(1)));
            },
            "View Albums"
        }
    };

    rsx! {
        div { class: "container",
            // Modal container for popups
            ModalBox { update_signal }

            // Page header
            div { class: "page-header",
                h1 { class: "section-title", "Photo Gallery" }
                p { "Browse and search all accessible media" }
            }

            // Search controls
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

            // Media grid
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
                                    MediaCard {
                                        key: "{media_uuid}",
                                        media_uuid: *media_uuid,
                                        show_actions: true,
                                    }
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
