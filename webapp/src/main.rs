#![allow(non_snake_case)]
use dioxus::prelude::*;
use dioxus_router::prelude::*;

use tracing::Level;

mod common;

mod components;
use components::navigation::NavBar;

mod home;
use home::ModernHome;

mod gallery;
use gallery::{Gallery, GalleryDetail, GallerySearch};

mod collection;
use collection::{CollectionDetail, CollectionSearch, Collections};

mod library;
use library::{Libraries, LibraryDetail, LibrarySearch};

fn main() {
    dioxus_logger::init(Level::DEBUG).expect("failed to init logger");
    launch(App);
}

// Probably the easiest way to make the gallery view useful is to
// have an optional ?collection=XXX,library=XXX when jumping back to the
// GalleryDetail for particular media
//
// this enables menus to say "came from this collection -- remove?" and
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
        #[nest("/collections")]
            #[layout(Collections)]
                #[route("/")]
                CollectionSearch {},
                #[route("/:collection_uuid")]
                CollectionDetail { collection_uuid: String },
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
        Router::<Route> { config: RouterConfig::default }
    }
}
