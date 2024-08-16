use dioxus::prelude::*;

use api::album::*;

pub async fn search_albums(req: &SearchAlbumsReq) -> anyhow::Result<SearchAlbumsResp> {
    Err(anyhow::Error::msg("not implemented"))
}

pub async fn search_media_in_album(
    req: &Option<SearchMediaInAlbumReq>,
) -> Option<anyhow::Result<SearchMediaInAlbumResp>> {
    Some(Err(anyhow::Error::msg("not implemented")))
}

// idea: drop down bar under subnav

// subnav returns to "browse albums" (i.e. search)

#[derive(Clone, PartialEq, Props)]
pub struct AlbumProps {}

// needs onclick to set view_album
#[component]
pub fn Album(props: AlbumProps) -> Element {
    rsx! {
        div {
            height: "400px",
            width: "400px",
            border: "5px solid #ffffff",
            display: "flex",
            flex_direction: "column",

            img {
                src: "/dev/null",
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct AlbumContentsProps {
    album_uuid: AlbumUuid,
}

#[derive(Clone, PartialEq, Props)]
pub struct AlbumContentBarProps {
    album_uuid: AlbumUuid,
}

// shows info about the current album along with the media search bar
#[component]
pub fn AlbumContentBar(props: AlbumContentBarProps) -> Element {
    rsx! { p { "todo" }}
}
// shows a grid of media (move to a common module)
#[component]
pub fn AlbumContents(props: AlbumContentsProps) -> Element {
    rsx! { p { "todo" }}
}

#[derive(Clone, PartialEq, Props)]
pub struct AlbumNavBarProps {
    search_albums_signal: Signal<SearchAlbumsReq>,
    search_albums_status: String,
}

// searches albums themselves and has the "clear album search result" button
#[component]
pub fn AlbumNavBar(props: AlbumNavBarProps) -> Element {
    rsx! { p { "todo" }}
}

// either shows a grid of albums (via search) or a content container (with its own search bar)
//
// clicking album should pop out side bar with details (that can be edited by owner)
//
// clicking create on NavBar should pop out different sidebar
#[component]
pub fn Albums() -> Element {
    let search_albums_signal = use_signal(|| SearchAlbumsReq {
        filter: String::from(""),
    });

    let search_albums_result =
        use_resource(move || async move { search_albums(&search_albums_signal()).await });

    let view_album = use_signal::<Option<AlbumUuid>>(|| None);

    let search_media_signal = use_signal::<Option<SearchMediaInAlbumReq>>(|| None);

    let search_media_result =
        use_resource(move || async move { search_media_in_album(&search_media_signal()).await });

    rsx! {
        span { "albums" }
    }
}
