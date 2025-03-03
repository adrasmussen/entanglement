use dioxus::prelude::*;

use crate::{
    common::storage::try_local_storage,
    components::{
        media_card::MediaCard,
        modal::{MODAL_STACK, Modal, ModalBox},
        search_bar::SearchBar,
    },
    gallery::MEDIA_SEARCH_KEY,
};
use api::media::*;

#[component]
pub fn GallerySearch() -> Element {
    let update_signal = use_signal(|| ());

    // Track whether advanced search options are expanded
    let mut advanced_expanded = use_signal(|| false);

    // Get search signal from local storage
    let media_search_signal = use_signal::<String>(|| try_local_storage(MEDIA_SEARCH_KEY));

    // Fetch media data
    let media_future = use_resource(move || async move {
        let filter = media_search_signal();
        search_media(&SearchMediaReq { filter }).await
    });

    // Create action button for search bar - now it's an "Advanced" toggle
    let action_button = rsx! {
        button {
            class: "btn btn-secondary",
            onclick: move |_| {
                advanced_expanded.set(!advanced_expanded());
            },
            "Advanced"
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

            // Advanced search options (expandable)
            {
                if advanced_expanded() {
                    rsx! {
                        div {
                            class: "advanced-search-options",
                            style: "
                                                                                                                                margin-top: -16px;
                                                                                                                                margin-bottom: var(--space-6);
                                                                                                                                padding: var(--space-4);
                                                                                                                                background-color: var(--neutral-50);
                                                                                                                                border-radius: 0 0 var(--radius-lg) var(--radius-lg);
                                                                                                                                box-shadow: var(--shadow-sm);
                                                                                                                                border-top: 1px solid var(--neutral-200);
                                                                                                                                animation: slide-down 0.2s ease-out;
                                                                                                                            ",
                            h3 { style: "margin-bottom: var(--space-3); font-size: 1rem;", "Advanced Search Options" }
                            div {
                                class: "coming-soon",
                                style: "
                                                                                                                                    display: flex;
                                                                                                                                    align-items: center;
                                                                                                                                    justify-content: center;
                                                                                                                                    padding: var(--space-6);
                                                                                                                                    background-color: var(--surface);
                                                                                                                                    border-radius: var(--radius-md);
                                                                                                                                    color: var(--text-secondary);
                                                                                                                                    font-style: italic;
                                                                                                                                ",
                                "Advanced search options coming soon..."
                            }
                        }
                    }
                } else {
                    rsx! {}
                }
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
