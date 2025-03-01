use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::{
    Route,
    common::{stream::*, style},
};
use api::media::*;

#[derive(Clone, PartialEq, Props)]
struct MediaTileProps {
    media_uuid: MediaUuid,
}

#[component]
fn MediaTile(props: MediaTileProps) -> Element {
    let media_uuid = props.media_uuid;

    rsx! {
        style { "{style::MEDIA_GRID}" }
        div {
            Link {
                class: "media-tile",
                to: Route::GalleryDetail {
                    media_uuid: media_uuid.to_string(),
                },
                img { src: thumbnail_link(media_uuid) }
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct MediaGridProps {
    media: Vec<MediaUuid>,
}

#[component]
pub fn MediaGrid(props: MediaGridProps) -> Element {
    rsx! {
        div {
            style { "{style::MEDIA_GRID}" }
            div { class: "media-grid",
                for media_uuid in props.media.iter() {
                    MediaTile { media_uuid: *media_uuid }
                }
            }
        }
    }
}
