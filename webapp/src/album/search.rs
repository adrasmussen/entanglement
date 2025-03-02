use dioxus::prelude::*;

use crate::components::modal::{Modal, ModalBox, MODAL_STACK};
use crate::{
    album::{table::AlbumTable, ALBUM_SEARCH_KEY},
    common::{storage::*, style},
};
use api::album::*;

// AlbumList elements
//
// this is the album search page
//
// clicking on an album focuses on the media in that album via AlbumDetail
#[derive(Clone, PartialEq, Props)]
struct AlbumSearchBarProps {
    album_search_signal: Signal<String>,
    status: String,
}

#[component]
fn AlbumSearchBar(props: AlbumSearchBarProps) -> Element {
    let mut album_search_signal = props.album_search_signal;
    let status = props.status;

    rsx! {
        div {
            div { class: "subnav",
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
                    }
                    input { r#type: "submit", value: "Search" }
                }
                span { "{status}" }
                button {
                    //onclick: move |_| { MODAL_STACK.with_mut(|v| v.push(Modal::CreateAlbum)) },
                    r#type: "button",
                    "Create album"
                }
            }
        }
    }
}

#[component]
pub fn AlbumSearch() -> Element {
    let update_signal = use_signal(|| ());

    let album_search_signal = use_signal::<String>(|| try_local_storage(ALBUM_SEARCH_KEY));

    let album_future = use_resource(move || async move {
        let filter = album_search_signal();

        search_albums(&SearchAlbumsReq { filter: filter }).await
    });

    let (albums, status) = match &*album_future.read() {
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

    rsx! {
        ModalBox { update_signal }
        AlbumSearchBar { album_search_signal, status }

        match albums {
            Ok(albums) => rsx! {
                AlbumTable { albums }
            },
            Err(err) => rsx! {
                span { "{err}" }
            },
        }
    }
}
