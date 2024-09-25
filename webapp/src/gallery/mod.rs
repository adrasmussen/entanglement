use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::{
    common::{storage::*, style, stream::*},
    Route,
};
use api::media::*;

pub mod grid;
use grid::MediaGrid;

const MEDIA_SEARCH_KEY: &str = "media_search";

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
            div {
                class: "subnav",
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

                    },
                    input {
                        r#type: "submit",
                        value: "Search",
                    },
                },
                span { "Search History" },
                span { "{status}" },
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
        GalleryListBar { media_search_signal: media_search_signal, status: status }

        match media {
            Ok(media) => rsx! {
                MediaGrid {media: media }
            },
            Err(err) => rsx! {
                span { "{err}" }
            },
        }
    }
}

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
            div {
                class: "subnav",
                span { "{status}" },
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct GalleryDetailProps {
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

    let status_signal = use_signal(|| String::from("waiting to update..."));

    let media_future = use_resource(move || async move {
        status_signal();

        get_media(&GetMediaReq {
            media_uuid: media_uuid,
        })
        .await
    });

    let (media, albums, tickets) = match &*media_future.read() {
        Some(Ok(resp)) => (
            resp.media.clone(),
            resp.albums.clone(),
            resp.tickets.clone(),
        ),
        Some(Err(err)) => return rsx! { span { "{err.to_string()}" } },
        None => return rsx! { span{ "Still waiting on get_media future..." } },
    };

    rsx! {
        GalleryDetailBar {status: status_signal()}

        div {
            style { "{style::GALLERY_DETAIL}" }
            div {
                class: "gallery-outer",
                // if we supported several types, we could match here, which would likely mean
                // geting a more general MediaDetail element or similar
                div {
                    img {
                        class: "gallery-img",
                        src: full_link(media_uuid)
                    }
                }
                div {
                    form {
                        class: "gallery-info",
                        onsubmit: move |event| async move {
                            let mut status_signal = status_signal;

                            let date = match event.values().get("date") {
                                Some(val) => val.as_value(),
                                None => String::from(""),
                            };

                            let note = match event.values().get("note") {
                                Some(val) => val.as_value(),
                                None => String::from(""),
                            };

                            let result = match update_media(&UpdateMediaReq {
                                media_uuid: media_uuid.clone(),
                                change: MediaMetadata {
                                    date: date,
                                    note: note,
                                }
                            }).await {
                                Ok(_) => String::from("Metadata updated successfully"),
                                Err(err) => format!("Error updating metadata: {}", err.to_string()),
                            };

                            status_signal.set(result)
                        },

                        label { "Library" },
                        span { "{media.library_uuid}" },

                        label { "Path" },
                        span { "{media.path}" },

                        label { "Hidden" },
                        span { "{media.hidden}" },

                        label { "Date" },
                        input {
                            name: "date",
                            r#type: "text",
                            value: "{media.metadata.date}"
                        },

                        label { "Note" },
                        textarea {
                            name: "note",
                            rows: "8",
                            value: "{media.metadata.note}"
                        },

                        input {
                            r#type: "submit",
                            value: "Update metadata",
                        }

                        div {
                            grid_column: "2",

                            button {
                                onclick: move |_| {},
                                r#type: "button",
                                "Create ticket"
                            }
                            button {
                                onclick: move |_| {},
                                r#type: "button",
                                "Albums"
                            }
                            button {
                                onclick: move |_| async move {
                                    let mut status_signal = status_signal;

                                    let result = match set_media_hidden(&SetMediaHiddenReq {
                                        media_uuid: media_uuid,
                                        hidden: !media.hidden,
                                    }).await {
                                        Ok(_) => String::from("Hidden state updated successfully"),
                                        Err(err) => format!("Error updating hidden state: {}", err.to_string()),

                                    };

                                    status_signal.set(result);
                                },
                                r#type: "button",
                                "Toggle Hidden"
                            }
                        }
                    }
                }
                div {
                    div {
                        class: "gallery-info",
                        span { "Albums: (needs name, link, and remove button, along with add modal)" }
                        for album_uuid in albums {
                            p { "{album_uuid}" }
                        }
                        span { "Tickets: (needs name, link, and mark resolved button, along with create modal)" }
                        for ticket_uuid in tickets {
                            p { "{ticket_uuid}" }
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
