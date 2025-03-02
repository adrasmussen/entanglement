use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::components::modal::{Modal, MODAL_STACK};
use crate::{common::style, Route};
use api::album::*;

#[derive(Clone, PartialEq, Props)]
struct AlbumTableRowProps {
    album_uuid: AlbumUuid,
}

#[component]
fn AlbumTableRow(props: AlbumTableRowProps) -> Element {
    let album_uuid = props.album_uuid;

    let album = use_resource(move || async move {
        get_album(&GetAlbumReq {
            album_uuid: album_uuid,
        })
        .await
    });

    let album = &*album.read();

    let result = match album {
        Some(Ok(result)) => result.album.clone(),
        _ => {
            return rsx! {
                tr {
                    span { "error fetching {album_uuid}" }
                }
            };
        }
    };

    rsx! {
        tr {
            td {
                Link {
                    to: Route::AlbumDetail {
                        album_uuid: album_uuid.to_string(),
                    },
                    span { "{result.name}" }
                }
            }
            td { "{result.uid}" }
            td { "{result.gid}" }
            td { "{result.note}" }
            td { "{result.mtime}" }
            td {
                button { float: "right", onclick: move |_| async move {}, "Update" }
                button { float: "right", onclick: move |_| async move {}, "Delete" }
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct AlbumListProps {
    albums: Vec<AlbumUuid>,
}

#[component]
pub fn AlbumTable(props: AlbumListProps) -> Element {
    rsx! {
        div {
            table {
                tr {
                    th { "Name" }
                    th { "Creator" }
                    th { "Group (TODO: group member modal)" }
                    th { "Note" }
                    th { "Last modified" }
                    th { "Operations" }
                }

                for album_uuid in props.albums.iter() {
                    AlbumTableRow { key: "{album_uuid}", album_uuid: *album_uuid }
                }
            }
        }
    }
}
