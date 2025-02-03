use dioxus::prelude::*;

use crate::{
    album::{grid::MediaGrid, MEDIA_SEARCH_KEY},
    common::{storage::*, style},
};
use api::album::*;

// AlbumDetail elements
//
// these are almost exactly the same as GalleryList, except that they call
// a different api call so as to restrict the results to a particular album
//
// the bar pulls double duty as both a search and status bar, but the body
// calls the same fundamental structure (even though the internals differ)
#[derive(Clone, PartialEq, Props)]
struct AlbumDetailBarProps {
    media_search_signal: Signal<String>,
    album_uuid: AlbumUuid,
    status: String,
}

#[component]
fn AlbumDetailBar(props: AlbumDetailBarProps) -> Element {
    let mut media_search_signal = props.media_search_signal;
    let album_uuid = props.album_uuid;
    let status = props.status;

    let album_future = use_resource(move || async move {
        get_album(&GetAlbumReq {
            album_uuid: album_uuid,
        })
        .await
    });

    let album = &*album_future.read();

    let album_result = match album {
        Some(Ok(result)) => result.album.clone(),
        _ => {
            return rsx! {
                div { class: "subnav",
                    span { "error fetching {album_uuid}" }
                }
            }
        }
    };

    rsx! {
        div {
            style { "{style::SUBNAV}" }
            div { class: "subnav",
                span { "Album: {album_result.name}" }
                span { "Owner: {album_result.uid}" }
                span { "Group: {album_result.gid}" }
                span { "MISSING: delete album modal, update album modal" }
            }
        }
        div {
            style { "{style::SUBNAV}" }
            div { class: "subnav",
                form {
                    onsubmit: move |event| async move {
                        let filter = match event.values().get("search_filter") {
                            Some(val) => val.as_value(),
                            None => String::from(""),
                        };
                        media_search_signal.set(filter.clone());
                        set_local_storage(MEDIA_SEARCH_KEY, filter);
                    },
                    input {
                        name: "search_filter",
                        r#type: "text",
                        value: "{media_search_signal()}",
                    }
                    input { r#type: "submit", value: "Search" }
                }
                span { "Search History" }
                span { "{status}" }
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct AlbumDetailProps {
    // this is a String because we get it from the Router
    album_uuid: String,
}

#[component]
pub fn AlbumDetail(props: AlbumDetailProps) -> Element {
    let album_uuid = match props.album_uuid.parse::<AlbumUuid>() {
        Ok(uuid) => uuid,
        Err(_) => {
            return rsx! {
                span { "failed to convert album_uuid" }
            }
        }
    };

    let media_search_signal = use_signal::<String>(|| try_local_storage(MEDIA_SEARCH_KEY));

    let media_future = use_resource(move || async move {
        let filter = media_search_signal();

        search_media_in_album(&SearchMediaInAlbumReq {
            album_uuid: album_uuid,
            filter: filter,
        })
        .await
    });

    let (media, status) = match &*media_future.read() {
        Some(Ok(resp)) => (
            Ok(resp.media.clone()),
            format!("Found {} results", resp.media.len()),
        ),
        Some(Err(err)) => (
            Err(err.to_string()),
            String::from("Error from search_media_in_album"),
        ),
        None => (
            Err(String::from(
                "Still waiting on search_media_in_album future...",
            )),
            String::from(""),
        ),
    };

    rsx!(
        AlbumDetailBar { media_search_signal, album_uuid, status }

        match media {
            Ok(media) => rsx! {
                MediaGrid { album_uuid, media }
            },
            Err(err) => rsx! {
                span { "{err}" }
            },
        }
    )
}
