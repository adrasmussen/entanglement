// webapp/src/album/grid.rs

use crate::album::card::AlbumCard;
use crate::components::modal::{Modal, MODAL_STACK};
use api::album::AlbumUuid;
use dioxus::prelude::*;

#[derive(Clone, PartialEq, Props)]
pub struct AlbumGridProps {
    albums: Vec<AlbumUuid>,
}

#[component]
pub fn AlbumGrid(props: AlbumGridProps) -> Element {
    let has_albums = !props.albums.is_empty();

    rsx! {
        if has_albums {
            div {
                class: "albums-grid",
                style: "
                    display: grid;
                    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
                    gap: var(--space-4);
                    margin-top: var(--space-4);
                ",
                for album_uuid in props.albums.iter() {
                    div { key: "{album_uuid}",
                        AlbumCard { album_uuid: *album_uuid }
                    }
                }
            }
        } else {
            div {
                class: "empty-state",
                style: "
                    padding: var(--space-8) var(--space-4);
                    text-align: center;
                    background-color: var(--surface);
                    border-radius: var(--radius-lg);
                    margin-top: var(--space-4);
                ",
                div { style: "
                        font-size: 4rem;
                        margin-bottom: var(--space-4);
                        color: var(--neutral-400);
                    ",
                    "📂"
                }
                h3 { style: "
                        margin-bottom: var(--space-2);
                        color: var(--text-primary);
                    ",
                    "No Albums Found"
                }
                p { style: "
                        color: var(--text-secondary);
                        max-width: 500px;
                        margin: 0 auto;
                    ",
                    "No albums match your search criteria. Try adjusting your search or create a new album to get started."
                }
                button {
                    class: "btn btn-primary",
                    style: "margin-top: var(--space-4);",
                    onclick: move |_| {
                        MODAL_STACK.with_mut(|v| v.push(Modal::CreateAlbum));
                    },
                    "Create New Album"
                }
            }
        }
    }
}
