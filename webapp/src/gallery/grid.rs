use dioxus::prelude::*;

use crate::common::{modal::Modal, stream::*, style};
use api::media::*;

#[derive(Clone, PartialEq, Props)]
struct MediaTileProps {
    modal_stack_signal: Signal<Vec<Modal>>,
    media_uuid: MediaUuid,
}

#[component]
fn MediaTile(props: MediaTileProps) -> Element {
    let mut modal_stack_signal = props.modal_stack_signal;
    let media_uuid = props.media_uuid;

    rsx! {
        style { "{style::MEDIA_GRID}" }
        div {
            class: "media-tile",
            img {
                onclick: move |_| { modal_stack_signal.push(Modal::ShowMedia(media_uuid)) },

                src: thumbnail_link(media_uuid),
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct MediaGridProps {
    modal_stack_signal: Signal<Vec<Modal>>,
    media: Vec<MediaUuid>,
}

#[component]
pub fn MediaGrid(props: MediaGridProps) -> Element {
    rsx! {
        div {
            style { "{style::MEDIA_GRID}" }
                div {
                    class: "media-grid",
                    for media_uuid in props.media.iter() {
                        MediaTile { modal_stack_signal: props.modal_stack_signal, media_uuid: *media_uuid }
                    }
                }
        }
    }
}
