use dioxus::prelude::*;
use tracing::debug;

use crate::{
    common::{modal::ModalBox, style},
    gallery::{info::MediaInfo, media::MediaView, related::MediaRelated},
};
use api::media::*;

// GalleryDetail elements
//
// upon clicking on a media thumbnail from anywhere, jump to this
// set of elements that displays the media details and has all of
// the api calls to modify those details
//
// once we support more media types, the main body will need to
// switch based on the MediaType enum
#[component]
fn GalleryDetailBar() -> Element {
    rsx! {
        div {
            style { "{style::SUBNAV}" }
            div { class: "subnav",
                span { "Gallery detail" }
            }
        }
    }
}

//
// ROUTE TARGET
//
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

    let update_signal = use_signal(|| ());

    let media_future = use_resource(move || async move {
        debug!("running get_media resource");
        update_signal.read();

        get_media(&GetMediaReq {
            media_uuid: media_uuid,
        })
        .await
    });

    // these three clones are not great, but it would take quite a bit of work to make the
    // destructure zero-copy given the types involved
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

    rsx! {
        ModalBox { update_signal }
        GalleryDetailBar {}

        div {
            style { "{style::GALLERY_DETAIL}" }
            div { class: "gallery-outer",
                MediaView { media_uuid, media_metadata: media.metadata.clone() }
                MediaInfo { update_signal, media_uuid, media }
                MediaRelated {
                    update_signal,
                    media_uuid,
                    albums,
                    comments,
                }
            }
        }
    }
}
