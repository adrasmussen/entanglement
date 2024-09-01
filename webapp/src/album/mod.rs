use dioxus::prelude::*;

use crate::common::{
    modal::{Modal, ModalBox},
    storage::*,
    style,
};
use crate::gallery::grid::MediaGrid;
use api::{album::*, media::MediaUuid};

mod list;
use list::AlbumList;

// states of the album page
#[derive(Clone)]
enum AlbumView {
    Pending,
    AlbumList(Vec<AlbumUuid>),
    MediaList(Vec<MediaUuid>),
    SearchError(String),
}

impl From<anyhow::Result<SearchAlbumsResp>> for AlbumView {
    fn from(sar: anyhow::Result<SearchAlbumsResp>) -> Self {
        match sar {
            Ok(resp) => AlbumView::AlbumList(resp.albums),
            Err(err) => AlbumView::SearchError(err.to_string()),
        }
    }
}

impl From<anyhow::Result<SearchMediaInAlbumResp>> for AlbumView {
    fn from(smiar: anyhow::Result<SearchMediaInAlbumResp>) -> Self {
        match smiar {
            Ok(resp) => AlbumView::MediaList(resp.media),
            Err(err) => AlbumView::SearchError(err.to_string()),
        }
    }
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
    select_album_signal: Signal<Option<AlbumUuid>>,
}

#[component]
fn AlbumNavBar(props: AlbumNavBarProps) -> Element {
    let mut album_view_signal = props.album_view_signal;
    let mut select_album_signal = props.select_album_signal;

    let album_search_filter = SearchAlbumsReq::fetch();
    let media_search_filter = SearchMediaInAlbumReq::fetch();

    // somewhat unfortunate hack because dioxus requires us to always call the hook,
    // even if we don't end up using the output
    //
    // note that this sets the album_view_signal but does not reset select_album
    let album = use_resource(move || async move {
        let album_uuid = match select_album_signal() {
            Some(val) => val,
            None => return None,
        };

        match get_album(&GetAlbumReq {
            album_uuid: album_uuid,
        })
        .await
        {
            Ok(resp) => return Some(resp.album),
            Err(err) => {
                album_view_signal.set(AlbumView::SearchError(format!(
                    "Failed to find album for {album_uuid}: {err}"
                )));
                return None;
            }
        }
    });

    rsx! {
        div {
            style { "{style::SUBNAV}" }
            div {
                class: "subnav",
                match select_album_signal() {
                    None => rsx! {
                        form {
                            onsubmit: move |event| async move {
                                let filter = match event.values().get("search_filter") {
                                    Some(val) => val.as_value(),
                                    None => String::from(""),
                                };

                                let req = SearchAlbumsReq{filter: filter};

                                album_view_signal.set(search_albums(&req).await.into());

                                req.store();
                            },
                            input {
                                name: "search_filter",
                                r#type: "text",
                                value: "{album_search_filter.filter}",

                            },
                            input {
                                r#type: "submit",
                                value: "Search",
                            },
                        },
                        span { "Search History" },
                        match album_view_signal() {
                            AlbumView::AlbumList(albums) => rsx! {
                                span { "Found {albums.len()} results" }
                            },
                            AlbumView::SearchError(_) => rsx! {
                                span { "Error while searching" }
                            },
                            _ => rsx!{}
                        },
                        button { "Create Album" },
                    },
                    Some(album_uuid) => {
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

                                    let req = SearchMediaInAlbumReq{album_uuid: album_uuid, filter: filter};

                                    album_view_signal.set(search_media_in_album(&req).await.into());

                                    req.store();
                                },
                                input {
                                    name: "media_search_filter",
                                    r#type: "text",
                                    value: "{media_search_filter.filter}",

                                },
                                input {
                                    r#type: "submit",
                                    value: "Search",
                                },
                                span { "Search History" }
                                match album_view_signal() {
                                    AlbumView::MediaList(media) => rsx! {
                                        span { "Searching {album_name}, found {media.len()} results" }
                                    },
                                    AlbumView::SearchError(_) => rsx! {
                                        span { "Error while searching" }
                                    },
                                    _ => rsx!{}
                                },
                                button { "View Album" },
                                button {
                                    onclick: move |_| select_album_signal.set(None),
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
    let album_view_signal = use_signal(|| AlbumView::Pending);
    let select_album_signal = use_signal::<Option<AlbumUuid>>(|| None);

    rsx! {
        AlbumNavBar { album_view_signal: album_view_signal, select_album_signal: select_album_signal}
        ModalBox { stack_signal: modal_stack_signal}

        match album_view_signal() {
            AlbumView::Pending => rsx! {
                span{"Search using the box above"}
            },
            AlbumView::AlbumList(albums) => rsx! {
                AlbumList { select_album_signal: select_album_signal, albums: albums }
            },
            AlbumView::MediaList(media) => rsx! {
                MediaGrid { modal_stack_signal: modal_stack_signal, media: media}
            },
            AlbumView::SearchError(err) => rsx! {
                span { "{err}" }
            },
        }
    }
}
