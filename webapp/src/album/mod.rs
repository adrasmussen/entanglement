use dioxus::prelude::*;

use crate::common::{
    modal::{Modal, ModalBox},
    storage::*,
    style,
};
use crate::gallery::grid::MediaGrid;
use api::album::*;

mod list;
use list::AlbumList;

pub const ALBUM_SEARCH_KEY: &str = "album_search";
pub const MEDIA_SEARCH_KEY: &str = "media_in_album_search";

// states of the album page
#[derive(Clone)]
enum AlbumView {
    AlbumList,
    MediaList(AlbumUuid),
}

impl SearchStorage for SearchAlbumsReq {
    fn store(&self) -> () {
        set_local_storage("album_search_req", &self)
    }

    fn fetch() -> Self {
        match get_local_storage("album_search_req") {
            Ok(val) => val,
            Err(_) => Self::default(),
        }
    }
}

impl SearchStorage for SearchMediaInAlbumReq {
    fn store(&self) -> () {
        set_local_storage("media_in_album_search_req", &self)
    }

    fn fetch() -> Self {
        match get_local_storage("media_in_album_search_req") {
            Ok(val) => val,
            Err(_) => Self::default(),
        }
    }
}

#[derive(Clone, PartialEq, Props)]
struct AlbumNavBarProps {
    album_view_signal: Signal<AlbumView>,
    album_search_signal: Signal<String>,
    media_search_signal: Signal<String>,
    status: (String, String),
}

#[component]
fn AlbumNavBar(props: AlbumNavBarProps) -> Element {
    let mut album_view_signal = props.album_view_signal;
    let mut album_search_signal = props.album_search_signal;
    let mut media_search_signal = props.media_search_signal;

    let (album_status, media_status) = props.status;

    // somewhat unfortunate hack because dioxus requires us to always call the hook,
    // even if we don't end up using the output
    //
    // TODO -- better logging for the Error
    let album = use_resource(move || async move {
        let album_uuid = match album_view_signal() {
            AlbumView::MediaList(val) => val,
            AlbumView::AlbumList => return None,
        };

        match get_album(&GetAlbumReq {
            album_uuid: album_uuid,
        })
        .await
        {
            Ok(resp) => return Some(resp.album),
            Err(_) => return None,
        }
    });

    rsx! {
        div {
            style { "{style::SUBNAV}" }
            div {
                class: "subnav",
                match album_view_signal() {
                    AlbumView::AlbumList => rsx! {
                        form {
                            onsubmit: move |event| async move {
                                let filter = match event.values().get("search_filter") {
                                    Some(val) => val.as_value(),
                                    None => String::from(""),
                                };

                                album_search_signal.set(filter.clone());

                                set_local_storage(ALBUM_SEARCH_KEY, filter);
                            },
                            input {
                                name: "search_filter",
                                r#type: "text",
                                value: "{album_search_signal()}",

                            },
                            input {
                                r#type: "submit",
                                value: "Search",
                            },
                        },
                        span { "Search History" },
                        span { "{album_status}"}
                        button { "Create Album" },
                    },
                    AlbumView::MediaList(_) => {
                        let album = &*album.read();

                        let album_name = match album.clone().flatten() {
                            Some(val) => val.metadata.name,
                            None => String::from("still waiting on get_album future...")
                        };

                        rsx! {
                            form {
                                onsubmit: move |event| async move {
                                    let filter = match event.values().get("media_search_filter") {
                                        Some(val) => val.as_value(),
                                        None => String::from(""),
                                    };

                                    media_search_signal.set(filter.clone());

                                    set_local_storage(MEDIA_SEARCH_KEY, filter);
                                },
                                input {
                                    name: "media_search_filter",
                                    r#type: "text",
                                    value: "{media_search_signal()}",

                                },
                                input {
                                    r#type: "submit",
                                    value: "Search",
                                },
                                span { "Search History" }
                                span { "Searching {album_name}: {media_status}" }
                                button { "View Album" },
                                button {
                                    onclick: move |_| album_view_signal.set(AlbumView::AlbumList),
                                    "Reset album search"
                                },
                            },
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn Albums() -> Element {
    let modal_stack_signal = use_signal::<Vec<Modal>>(|| Vec::new());
    let album_view_signal = use_signal(|| AlbumView::AlbumList);

    // album search logic
    let album_search_signal = use_signal::<String>(|| try_local_storage(ALBUM_SEARCH_KEY));
    let album_future = use_resource(move || async move {
        let filter = album_search_signal();

        search_albums(&SearchAlbumsReq { filter: filter }).await
    });

    let (albums, album_status) = match &*album_future.read() {
        Some(Ok(resp)) => (
            Ok(resp.albums.clone()),
            format!("Found {} results", resp.albums.len()),
        ),
        Some(Err(err)) => (
            Err(err.to_string()),
            String::from("Error from search_albums"),
        ),
        None => (
            Err(String::from("Still waiting on search_albums future...")),
            String::from(""),
        ),
    };

    // media search logic
    let media_search_signal = use_signal::<String>(|| try_local_storage(MEDIA_SEARCH_KEY));
    let media_future = use_resource(move || async move {
        let album_uuid = match album_view_signal() {
            AlbumView::MediaList(album_uuid) => album_uuid,
            AlbumView::AlbumList => {
                return Err(anyhow::Error::msg(
                    "album_uuid not specified in media_future",
                ))
            }
        };

        let filter = media_search_signal();

        search_media_in_album(&SearchMediaInAlbumReq {
            album_uuid: album_uuid,
            filter: filter,
        })
        .await
    });

    let (media, media_status) = match &*media_future.read() {
        Some(Ok(resp)) => (
            Ok(resp.media.clone()),
            format!("Found {} results", resp.media.len()),
        ),
        Some(Err(err)) => (
            Err(err.to_string()),
            String::from("Error from search_media_in_albums"),
        ),
        None => (
            Err(String::from(
                "Still waiting on search_media_in_albums future...",
            )),
            String::from(""),
        ),
    };

    rsx! {
        AlbumNavBar {
            album_view_signal: album_view_signal,
            album_search_signal: album_search_signal,
            media_search_signal: media_search_signal,
            status: (album_status, media_status),
        }
        ModalBox { stack_signal: modal_stack_signal }

        match album_view_signal() {
            AlbumView::AlbumList => match albums {
                Ok(albums) => rsx! {
                    AlbumList { album_view_signal: album_view_signal, albums: albums }
                },
                Err(err) => rsx! {
                    span { "{err}" }
                },
            },
            AlbumView::MediaList(_) => match media {
                Ok(media) => rsx! {
                    MediaGrid { modal_stack_signal: modal_stack_signal, media: media}
                },
                Err(err) => rsx! {
                    span { "{err}" }
                },
            },
        }
    }
}
