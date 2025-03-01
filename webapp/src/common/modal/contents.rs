use dioxus::prelude::*;

use crate::common::{
    local_time,
    modal::{modal_err, MODAL_STACK},
};
use api::{album::*, media::MediaUuid};

#[derive(Clone, PartialEq, Props)]
pub struct AddMediaToAlbumBoxProps {
    media_uuid: MediaUuid,
    album_uuid: AlbumUuid,
}

#[component]
pub fn AddMediaToAlbumBox(props: AddMediaToAlbumBoxProps) -> Element {
    let media_uuid = props.media_uuid;
    let album_uuid = props.album_uuid;

    let status_signal = use_signal(|| String::from(""));

    rsx! {
        div { class: "modal-body", grid_template_columns: "1fr",
            div {
                h3 { "Add media to an album" }
                span { "not implemented" }
            }

        }
        div { class: "modal-footer",
            span { "{status_signal}" }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct AddMediaToAnyAlbumBoxProps {
    media_uuid: MediaUuid,
}

#[component]
pub fn AddMediaToAnyAlbum(props: AddMediaToAnyAlbumBoxProps) -> Element {
    let media_uuid = props.media_uuid;

    let status_signal = use_signal(|| String::from(""));

    rsx! {
        div { class: "modal-body", grid_template_columns: "1fr",
            div {
                h3 { "Add media to any album" }
                span { "not implemented" }
            }

        }
        div { class: "modal-footer",
            span { "{status_signal}" }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct RmMediaFromAlbumBoxProps {
    media_uuid: MediaUuid,
    album_uuid: AlbumUuid,
}

#[component]
pub fn RmMediaFromAlbum(props: RmMediaFromAlbumBoxProps) -> Element {
    let media_uuid = props.media_uuid;
    let album_uuid = props.album_uuid;

    let status_signal = use_signal(|| String::from(""));

    rsx! {
        div { class: "modal-body", grid_template_columns: "1fr",
            div {
                h3 { "Remove media from an album" }
                span { "not implemented" }
            }

        }
        div { class: "modal-footer",
            span { "{status_signal}" }
        }
    }
}
