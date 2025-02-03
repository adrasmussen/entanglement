use dioxus::prelude::*;

use crate::{
    common::style,
    gallery::{info::MediaInfo, media::MediaDetail, related::MediaRelated},
};
use api::media::*;

// GalleryDetail elements
//
// upon clicking on a media thumbnail from anywhere, jump to this
// set of elements that displays the media details and has all of
// the api calls to modify those details
//
// we will eventually want to add in a ?-search string to the path
// to keep some of the context of where we came from
//
// once we support more media types, the main body will need to
// switch based on the MediaType enum
#[derive(Clone, PartialEq, Props)]
struct GalleryDetailBarProps {
    status: String,
}

#[component]
fn GalleryDetailBar(props: GalleryDetailBarProps) -> Element {
    let status = props.status;

    rsx! {
        div {
            style { "{style::SUBNAV}" }
            div { class: "subnav",
                span { "{status}" }
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct GalleryDetailProps {
    // this is a String because we get it from the Router
    media_uuid: String,
}

#[component]
pub fn GalleryDetail(props: GalleryDetailProps) -> Element {
    let media_uuid = match props.media_uuid.parse::<MediaUuid>() {
        Ok(uuid) => uuid,
        Err(_) => {
            return rsx! {
                span { "failed to convert media_uuid" }
            }
        }
    };

    // with some better ergonomics, we might be able to remove this
    let status_signal = use_signal(|| String::from("waiting to update..."));

    let media_future = use_resource(move || async move {
        status_signal();

        get_media(&GetMediaReq {
            media_uuid: media_uuid,
        })
        .await
    });

    let (media, albums, comments) = match &*media_future.read() {
        Some(Ok(resp)) => (
            resp.media.clone(),
            resp.albums.clone(),
            resp.comments.clone(),
        ),
        Some(Err(err)) => {
            return rsx! {
                span { "err: {err.to_string()}" }
            }
        }
        None => {
            return rsx! {
                span { "Still waiting on get_media future..." }
            }
        }
    };

    // this whole thing should probably be carved up into three elements:
    //  1) display box (per media)
    //  2) edit functions (has media-specific extras)
    //  3) comments
    rsx! {
        GalleryDetailBar { status: status_signal() }

        div {
            style { "{style::GALLERY_DETAIL}" }
            div { class: "gallery-outer",
                MediaDetail { media_uuid, media_type: media.metadata.clone() }
                MediaInfo { media_uuid, media, status_signal }
                MediaRelated { albums, comments }
            }
        }
    }
}
