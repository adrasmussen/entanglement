use dioxus::prelude::*;

use crate::common::stream::*;
use api::media::*;

#[derive(Clone, PartialEq, Props)]
pub struct MediaViewProps {
    media_uuid: MediaUuid,
    media_metadata: MediaMetadata,
}

#[component]
pub fn MediaView(props: MediaViewProps) -> Element {
    rsx! {
        div {
            match props.media_metadata {
                MediaMetadata::Image => {
                    let image_src = full_link(props.media_uuid);
                    rsx! {
                        a { href: image_src.clone(), target: "_blank",
                            img { class: "gallery-media", src: image_src }
                        }
                    }
                }
                _ => rsx! {
                    span { "media type not supported" }
                },
            }
        }
    }
}
