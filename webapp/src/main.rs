#![allow(non_snake_case)]
use dioxus::prelude::*;
use dioxus_router::prelude::*;

use tracing::{info, Level};

mod common;
use common::style;

mod home;
use home::Home;

mod gallery;
use gallery::Gallery;

mod album;
use album::Albums;

mod ticket;
use ticket::Tickets;

mod library;
use library::Libraries;

mod settings;
use settings::Settings;

mod status;
use status::Status;

mod admin;
use admin::Admin;

fn main() {
    // Init logger
    dioxus_logger::init(Level::INFO).expect("failed to init logger");
    launch(App);
}

#[derive(Clone, PartialEq, Routable)]
#[rustfmt::skip]
enum Route {
    #[layout(NavBar)]
        #[route("/")]
        Home {},
        #[route("/gallery")]
        Gallery {},
        #[route("/albums")]
        Albums {},
        #[route("/library")]
        Libraries {},
        #[route("/tickets")]
        Tickets {},
        #[route("/settings")]
        Settings {},
        #[route("/status")]
        Status {},
        #[route("/admin")]
        Admin {},
}

#[component]
pub fn App() -> Element {
    rsx! { Router::<Route> { config: move || RouterConfig::default().history(WebHistory::default())} }
}

#[component]
fn NavBar() -> Element {
    rsx! {
        div {
            style { "{style::TOPNAV}" },
            div {class: "topnav",
                Link { active_class: "active", to: Route::Home {}, "Home" }
                Link { active_class: "active", to: Route::Gallery {}, "Gallery" }
                Link { active_class: "active", to: Route::Albums {}, "Albums" }
                Link { active_class: "active", to: Route::Libraries {}, "Libraries" }
                Link { active_class: "active", to: Route::Tickets {}, "Tickets" }
                Link { active_class: "active", to: Route::Settings {}, "Settings" }
                Link { active_class: "active", to: Route::Status {}, "Status" }
                Link { active_class: "active", to: Route::Admin {}, "Admin" }
            }
        }
        Outlet::<Route> {}
    }
}
