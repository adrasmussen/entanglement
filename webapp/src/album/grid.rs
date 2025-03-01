use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::{
    Route,
    common::{storage::set_local_storage, stream::*, style},
    gallery::GALLERY_ALBUM_KEY,
};
use api::{album::AlbumUuid, media::*};

#[derive(Clone, PartialEq, Props)]
struct MediaTileProps {
    album_uuid: AlbumUuid,
    media_uuid: MediaUuid,
}

#[component]
fn MediaTile(props: MediaTileProps) -> Element {
    let album_uuid = props.album_uuid;
    let media_uuid = props.media_uuid;

    rsx! {
        style { "{style::MEDIA_GRID}" }
        div {
            Link {
                class: "media-tile",
                to: Route::GalleryDetail {
                    media_uuid: media_uuid.to_string(),
                },
                onclick: move |_| {
                    set_local_storage(GALLERY_ALBUM_KEY, album_uuid.to_string());
                },
                img { src: thumbnail_link(media_uuid) }
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct MediaGridProps {
    album_uuid: AlbumUuid,
    media: Vec<MediaUuid>,
}

#[component]
pub fn MediaGrid(props: MediaGridProps) -> Element {
    let album_uuid = props.album_uuid;

    rsx! {
        div {
            style { "{style::MEDIA_GRID}" }
            div { class: "media-grid",
                for media_uuid in props.media.iter() {
                    MediaTile { album_uuid, media_uuid: *media_uuid }
                }
            }
        }
    }
}
