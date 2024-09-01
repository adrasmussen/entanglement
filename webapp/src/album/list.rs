use dioxus::prelude::*;

use crate::common::style;
use api::album::*;

#[derive(Clone, PartialEq, Props)]
struct AlbumListEntryProps {
    select_album_signal: Signal<Option<AlbumUuid>>,
    album_uuid: AlbumUuid,
}

#[component]
fn AlbumListEntry(props: AlbumListEntryProps) -> Element {
    let mut select_album_signal = props.select_album_signal;
    let album_uuid = props.album_uuid;

    let album = use_resource(move || async move {
        get_album(&GetAlbumReq {
            album_uuid: album_uuid,
        })
        .await
    });

    let album = &*album.read();

    // this should throw a more informative error
    let result = match album {
        Some(Ok(result)) => result.album.clone(),
        _ => return rsx! {}
    };

    rsx! {
            tr {
                onclick: move |_| { select_album_signal.set(Some(album_uuid)) },
                td { "{result.uid}" }
                td { "{result.gid}" }
                td { "{result.metadata.name}" }
                td { "{result.metadata.note}" }
            }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct AlbumListProps {
    select_album_signal: Signal<Option<AlbumUuid>>,
    albums: Vec<AlbumUuid>
}

#[component]
pub fn AlbumList(props: AlbumListProps) -> Element {
    rsx! {
        div {
            style { "{style::TABLE}" }
                table {
                    tr {
                        th { "Creator" }
                        th { "Group" }
                        th { "Name" }
                        th { "Note" }
                    }

                    for album_uuid in props.albums.iter() {
                        AlbumListEntry { select_album_signal: props.select_album_signal, album_uuid: *album_uuid }
                    }
                }
        }
    }
}
