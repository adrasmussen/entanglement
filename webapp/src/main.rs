#![allow(non_snake_case)]
use dioxus::prelude::*;
use dioxus_router::prelude::*;

use tracing::{info, Level};

mod home;
use home::Home;

mod gallery;
use gallery::Gallery;

mod albums;
use albums::Albums;

mod library;
use library::Library;

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

// ANCHOR: router
#[derive(Routable, Clone)]
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
        Library {},
        #[route("/settings")]
        Settings {},
        #[route("/status")]
        Status {},
        #[route("/admin")]
        Admin {}

//    #[end_layout]
}
// ANCHOR_END: router

#[component]
pub fn App() -> Element {
    rsx! { Router::<Route> {} }
}

#[component]
fn NavBar() -> Element {
    let style = r#"
        .topnav {
            overflow: hidden;
            background-color: #e9e9e9;
        }

        .topnav span {
            float: left;
            display: block;
            color: black;
            text-align: center;
            padding: 14px 16px;
            text-decoration: none;
            font-size: 17px;
        }

        .topnav span:hover {
            background-color: #ddd;
            color: black;
        }

        .topnav span.active {
            background-color: #2196F3;
            color: white;
        }
    "#;

    rsx! {
        div {
            style { "{style}" },
            div {class: "topnav",
                span { Link { to: Route::Home {}, "Home" } }
                span { Link { to: Route::Gallery {}, "Gallery" } }
                span { Link { to: Route::Albums {}, "Albums" } }
                span { Link { to: Route::Library {}, "Library" } }
                span { Link { to: Route::Settings {}, "Settings" } }
                span { Link { to: Route::Status {}, "Status" } }
                span { Link { to: Route::Admin {}, "Admin" } }
            }
        }
        Outlet::<Route> {}
    }
}
