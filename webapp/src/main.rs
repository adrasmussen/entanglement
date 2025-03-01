#![allow(non_snake_case)]
use dioxus::prelude::*;
use dioxus_router::prelude::*;

use tracing::Level;

mod common;
use common::style;

mod components;

mod home;
use home::ModernHome;

mod gallery;
use gallery::{Gallery, GalleryDetail, GallerySearch};

mod album;
use album::{AlbumDetail, AlbumSearch, Albums};

mod library;
use library::{Libraries, LibraryDetail, LibrarySearch};

fn main() {
    // Init logger
    dioxus_logger::init(Level::DEBUG).expect("failed to init logger");
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
        ModernHome {},
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
        style { "{common::style::MODERN_STYLES}" }
        style { "{common::style::HOME_STYLES}" }
        Router::<Route> { config: move || RouterConfig::default() }
    }
}

// Replace the NavBar component with our modern version
#[component]
fn NavBar() -> Element {
    rsx! {
        components::navigation::ModernNavBar {}
        Outlet::<Route> {}
    }
}
