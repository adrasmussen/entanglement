use dioxus::prelude::*;

use crate::common::stream::*;
use api::media::*;

#[derive(Clone, PartialEq, Props)]
pub struct MediaDetailProps {
    media_uuid: MediaUuid,
    media_type: MediaMetadata,
}

#[component]
pub fn MediaDetail(props: MediaDetailProps) -> Element {
    rsx! {
        div { class: "gallery-media",
            match props.media_type {
                MediaMetadata::Image => {
                    let image_src = full_link(props.media_uuid);
                    rsx! {
                        div {
                            a { href: image_src.clone(), target: "_blank",
                                img { src: image_src }
                            }
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
