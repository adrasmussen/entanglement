use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::{
    common::{storage::*, stream::*, style},
    Route,
};
use common::api::media::*;

pub mod grid;
use grid::MediaGrid;

const MEDIA_SEARCH_KEY: &str = "media_search";

// GalleryList elements
//
// this group of elements allows users to search all media that they
// can access by calling search_media(), and displays the results in
// a MediaGrid
//
// clicking on a grid tile jumps to GalleryDetail
#[derive(Clone, PartialEq, Props)]
struct GalleryListBarProps {
    media_search_signal: Signal<String>,
    status: String,
}

#[component]
fn GalleryListBar(props: GalleryListBarProps) -> Element {
    let mut media_search_signal = props.media_search_signal;
    let status = props.status;

    rsx! {
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
                        value: "{media_search_signal()}"
                    }
                    input { r#type: "submit", value: "Search" }
                }
                span { "Search History" }
                span { "{status}" }
                span { "MISSING: needs attention checkbox" }
            }
        }
    }
}

#[component]
pub fn GalleryList() -> Element {
    let media_search_signal = use_signal::<String>(|| try_local_storage(MEDIA_SEARCH_KEY));

    let media_future = use_resource(move || async move {
        let filter = media_search_signal();

        search_media(&SearchMediaReq { filter: filter }).await
    });

    let (media, status) = match &*media_future.read() {
        Some(Ok(resp)) => (
            Ok(resp.media.clone()),
            format!("Found {} results", resp.media.len()),
        ),
        Some(Err(err)) => (
            Err(err.to_string()),
            String::from("Error from search_media"),
        ),
        None => (
            Err(String::from("Still waiting on search_media future...")),
            String::from(""),
        ),
    };

    rsx! {
        GalleryListBar { media_search_signal, status }

        match media {
            Ok(media) => rsx! {
                MediaGrid { media: media }
            },
            Err(err) => rsx! {
                span { "{err}" }
            },
        }
    }
}

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
                span { "{err.to_string()}" }
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
                // if we supported several types, we could match here, which would likely mean
                // geting a more general MediaDetail element or similar
                div {
                    img { class: "gallery-img", src: full_link(media_uuid) }
                }
                div {
                    form {
                        class: "gallery-info",
                        onsubmit: move |event| async move {
                            let mut status_signal = status_signal;
                            let date = event.values().get("date").map(|v| v.as_value());
                            let note = event.values().get("note").map(|v| v.as_value());
                            let result = match update_media(
                                    &UpdateMediaReq {
                                        media_uuid: media_uuid.clone(),
                                        update: MediaUpdate {
                                            hidden: None,
                                            attention: None,
                                            date: date,
                                            note: note,
                                        },
                                    },
                                )
                                .await
                            {
                                Ok(_) => String::from("Metadata updated successfully"),
                                Err(err) => format!("Error updating metadata: {}", err.to_string()),
                            };
                            status_signal.set(result)
                        },

                        label { "Library" }
                        span { "{media.library_uuid}" }

                        label { "Path" }
                        span { "{media.path}" }

                        label { "Hidden" }
                        span { "{media.hidden}" }

                        label { "Needs attention" }
                        span { "{media.attention}" }

                        label { "Date" }
                        input {
                            name: "date",
                            r#type: "text",
                            value: "{media.date}"
                        }

                        label { "Note" }
                        textarea { name: "note", rows: "8", value: "{media.note}" }

                        input { r#type: "submit", value: "Update metadata" }
                    }
                    div { grid_column: "2",

                        button { onclick: move |_| {}, r#type: "button", "Create comment" }
                        button { onclick: move |_| {}, r#type: "button", "Add to album" }
                        button { onclick: move |_| {}, r#type: "button", "Remove from album" }
                        button {
                            onclick: move |_| async move {
                                let mut status_signal = status_signal;
                                let result = match update_media(
                                        &UpdateMediaReq {
                                            media_uuid: media_uuid,
                                            update: MediaUpdate {
                                                hidden: Some(!media.hidden),
                                                attention: None,
                                                date: None,
                                                note: None,
                                            },
                                        },
                                    )
                                    .await
                                {
                                    Ok(_) => String::from("Hidden state updated successfully"),
                                    Err(err) => format!("Error updating hidden state: {}", err.to_string()),
                                };
                                status_signal.set(result);
                            },
                            r#type: "button",
                            "Toggle Hidden"
                        }
                        button { onclick: move |_| {}, r#type: "button", "Needs attention" }
                    }

                    div {
                        span { "MISSING: magic callback logic" }
                    }
                }
                div {
                    div { class: "gallery-info",
                        span { "Albums: MISSING (needs name, owner, group)" }
                        for album_uuid in albums {
                            p { "{album_uuid}" }
                        }
                        span { "Comments: MISSING (needs comment blocks here)" }
                        for comment_uuid in comments {
                            p { "{comment_uuid}" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn Gallery() -> Element {
    rsx! {
        Outlet::<Route> {}
    }
}
