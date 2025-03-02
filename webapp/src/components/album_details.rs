use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::{
    Route,
    components::modal::{MODAL_STACK, Modal},
};
use api::album::*;
use api::media::MediaUuid;

#[derive(Clone, PartialEq, Props)]
pub struct AlbumDetailsTableProps {
    album_uuids: Signal<Vec<AlbumUuid>>,
    media_uuid: MediaUuid,
    update_signal: Signal<()>,
}

#[component]
pub fn AlbumDetailsTable(props: AlbumDetailsTableProps) -> Element {
    let album_uuids = props.album_uuids;
    let media_uuid = props.media_uuid;
    let _update_signal = props.update_signal;

    // Fetch details for each album
    let albums_future = use_resource(move || {
        async move {
            let mut albums = Vec::new();

            for album_uuid in album_uuids() {
                match get_album(&GetAlbumReq { album_uuid }).await {
                    Ok(resp) => albums.push((album_uuid, resp.album)),
                    Err(err) => {
                        tracing::error!("Failed to fetch album {album_uuid}: {err}");
                    }
                }
            }

            // Sort albums by name for better display
            albums.sort_by(|a, b| a.1.name.cmp(&b.1.name));
            albums
        }
    });

    let albums = &*albums_future.read();

    let albums = albums.clone();

    rsx! {
        div {
            class: "detail-section",
            style: "background-color: var(--surface); padding: var(--space-4); border-radius: var(--radius-lg); box-shadow: var(--shadow-sm);",
            div {
                class: "section-header",
                style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: var(--space-3);",
                h2 { "Albums" }
                button {
                    class: "btn btn-sm btn-secondary",
                    onclick: move |_| {
                        MODAL_STACK.with_mut(|v| v.push(Modal::AddMediaToAnyAlbum(media_uuid)));
                    },
                    "Add to Album"
                }
            }

            match albums {
                Some(albums) => {
                    if albums.is_empty() {
                        rsx! {
                            div {
                                class: "empty-state",
                                style: "padding: var(--space-3); text-align: center; color: var(--text-tertiary);",
                                "This media is not in any albums."
                            }
                        }
                    } else {
                        rsx! {
                            div {
                                class: "table-container",
                                style: "max-height: 250px; overflow-y: auto; border: 1px solid var(--border); border-radius: var(--radius-md);",
                                table { style: "border-collapse: separate; border-spacing: 0;",
                                    thead { style: "position: sticky; top: 0; z-index: 1; background-color: var(--primary);",
                                        tr {
                                            th { "Name" }
                                            th { "Group" }
                                            th { "Note" }
                                            th { style: "width: 100px;", "Actions" }
                                        }
                                    }
                                    tbody {
                                        for (album_id , album) in albums.clone() {
                                            tr {
                                                td { style: "padding: var(--space-2) var(--space-3);",
                                                    Link {
                                                        to: Route::AlbumDetail {
                                                            album_uuid: album_id.to_string(),
                                                        },
                                                        style: "font-weight: 500; color: var(--primary);",
                                                        "{album.name}"
                                                    }
                                                }
                                                td { style: "padding: var(--space-2) var(--space-3);", "{album.gid}" }
                                                td {
                                                    style: "max-width: 200px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; padding: var(--space-2) var(--space-3);",
                                                    title: "{album.note}",
                                                    if album.note.is_empty() {
                                                        "(No description)"
                                                    } else {
                                                        "{album.note}"
                                                    }
                                                }
                                                td { style: "text-align: right; padding: var(--space-2) var(--space-3);",
                                                    button {
                                                        class: "btn btn-sm btn-danger",
                                                        onclick: move |_| {
                                                            MODAL_STACK.with_mut(|v| v.push(Modal::RmMediaFromAlbum(media_uuid, album_id)));
                                                        },
                                                        "Remove"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            div { style: "text-align: right; margin-top: var(--space-2); font-size: 0.875rem; color: var(--text-tertiary);",
                                "Showing {albums.len()} album"
                                if albums.len() != 1 {
                                    "s"
                                } else {
                                    ""
                                }
                            }
                        }
                    }
                }
                None => {
                    rsx! {
                        div { class: "loading-state",
                            for _ in 0..3 {
                                div { class: "skeleton", style: "height: 36px; margin-bottom: 8px;" }
                            }
                        }
                    }
                }
            }
        }
    }
}
