#![allow(non_snake_case)]
use dioxus::prelude::*;
use dioxus_router::prelude::*;

use tracing::{info, Level};

mod common;
use common::style;

mod home;
use home::Home;

mod gallery;
use gallery::{Gallery, GalleryDetail, GallerySearch};

mod album;
use album::{AlbumDetail, AlbumSearch, Albums};

mod library;
use library::{Libraries, LibraryDetail, LibrarySearch};

fn main() {
    // Init logger
    dioxus_logger::init(Level::INFO).expect("failed to init logger");
    launch(App);
}

// Probably the easiest way to make the gallery view useful is to
// have an optional ?album=XXX,library=XXX when jumping back to the
// GalleryDetail for particular media
//
// this enables menus to say "came from this album -- remove?" and
// possibly other things

#[derive(Clone, PartialEq, Routable)]
#[rustfmt::skip]
enum Route {
    #[layout(NavBar)]
        #[route("/")]
        Home {},
        #[nest("/gallery")]
            #[layout(Gallery)]
                #[route("/")]
                GallerySearch {},
                #[route("/:media_uuid")]
                GalleryDetail { media_uuid: String },
            #[end_layout]
        #[end_nest]
        #[nest("/albums")]
            #[layout(Albums)]
                #[route("/")]
                AlbumSearch {},
                #[route("/:album_uuid")]
                AlbumDetail { album_uuid: String },
            #[end_layout]
        #[end_nest]
        #[nest("/library")]
            #[layout(Libraries)]
                #[route("/")]
                LibrarySearch {},
                #[route("/:library_uuid")]
                LibraryDetail { library_uuid: String },
}

#[component]
pub fn App() -> Element {
    rsx! {
        Router::<Route> { config: move || RouterConfig::default() }
    }
}

#[component]
fn NavBarButton(target: Route, text: String) -> Element {
    let current_path: Route = use_route();

    let active_class = if current_path.is_child_of(&target) || current_path == target {
        "active"
    } else {
        ""
    };

    rsx! {
        Link { class: active_class, to: target, "{text}" }
    }
}

#[component]
fn NavBar() -> Element {
    rsx! {
        div {
            style { "{style::TOPNAV}" }
            div { class: "topnav",
                Link { active_class: "active", to: Route::Home {}, "Home" }
                NavBarButton { target: Route::GallerySearch {}, text: "Gallery" }
                NavBarButton { target: Route::AlbumSearch {}, text: "Albums" }
                NavBarButton { target: Route::LibrarySearch {}, text: "Libraries" }
            }
        }
        Outlet::<Route> {}
    }
}
