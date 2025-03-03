use dioxus::prelude::*;

use crate::{
    album::{ALBUM_SEARCH_KEY, grid::AlbumGrid},
    common::storage::*,
    components::{
        modal::{MODAL_STACK, Modal, ModalBox},
        search_bar::SearchBar,
    },
};
use api::album::*;

#[component]
pub fn AlbumSearch() -> Element {
    let update_signal = use_signal(|| ());

    // Get search signal from local storage
    let album_search_signal = use_signal::<String>(|| try_local_storage(ALBUM_SEARCH_KEY));

    // Fetch albums data
    let album_future = use_resource(move || async move {
        let filter = album_search_signal();
        search_albums(&SearchAlbumsReq { filter }).await
    });

    // Create action button for search bar - positioned on the right
    let action_button = rsx! {
        div { style: "margin-left: auto;", // This will push the button to the right
            button { class: "btn btn-primary", onclick: move |_| {}, "Create Album" }
        }
    };

    // Get status text
    let status = match &*album_future.read() {
        Some(Ok(resp)) => format!("Found {} albums", resp.albums.len()),
        Some(Err(_)) => String::from("Error searching albums"),
        None => String::from("Loading..."),
    };

    rsx! {
        div { class: "container",
            // Modal container for popups
            ModalBox { update_signal }

            // Page header
            div { class: "page-header", style: "margin-bottom: var(--space-4);",
                h1 { class: "section-title", "Albums" }
                p { "Organize and browse your media collections" }
            }

            // Search controls
            SearchBar {
                search_signal: album_search_signal,
                storage_key: ALBUM_SEARCH_KEY,
                placeholder: "Search by album name or description...",
                status,
                action_button,
            }

            // Album grid
            match &*album_future.read() {
                Some(Ok(resp)) => {
                    rsx! {
                        AlbumGrid { albums: resp.albums.clone() }
                    }
                }
                Some(Err(err)) => rsx! {
                    div {
                        class: "error-state",
                        style: "
                                                padding: var(--space-4);
                                                background-color: var(--surface);
                                                border-radius: var(--radius-lg);
                                                margin-top: var(--space-4);
                                                color: var(--error);
                                                text-align: center;
                                            ",
                        "Error: {err}"
                    }
                },
                None => rsx! {
                    div {
                        class: "loading-state albums-grid",
                        style: "
                                                display: grid;
                                                grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
                                                gap: var(--space-4);
                                                margin-top: var(--space-4);
                                            ",
                        for _ in 0..6 {
                            div {
                                class: "album-card loading",
                                style: "
                                                        background-color: var(--surface);
                                                        border-radius: var(--radius-lg);
                                                        overflow: hidden;
                                                        box-shadow: var(--shadow-sm);
                                                        height: 100%;
                                                    ",

                                // Skeleton loading UI
                                div { class: "skeleton", style: "height: 180px;" }

                                div { style: "padding: var(--space-3);",

                                    div {
                                        class: "skeleton",
                                        style: "width: 70%; height: 24px; margin-bottom: var(--space-2);",
                                    }

                                    div {
                                        class: "skeleton",
                                        style: "width: 100%; height: 16px; margin-bottom: var(--space-1);",
                                    }

                                    div {
                                        class: "skeleton",
                                        style: "width: 90%; height: 16px; margin-bottom: var(--space-3);",
                                    }

                                    div { style: "display: flex; justify-content: flex-end; gap: var(--space-2);",
                                        div {
                                            class: "skeleton",
                                            style: "width: 40px; height: 24px;",
                                        }
                                        div {
                                            class: "skeleton",
                                            style: "width: 40px; height: 24px;",
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
            }
        }
    }
}
