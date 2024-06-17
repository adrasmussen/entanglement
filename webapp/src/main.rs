#![allow(non_snake_case)]

use dioxus::prelude::*;
use tracing::{info, Level};

mod nav;
use nav::{AppPages, AppNavBar, AppMainPage};

mod gallery;

fn main() {
    // Init logger
    dioxus_logger::init(Level::INFO).expect("failed to init logger");
    launch(App);
}

#[component]
fn App() -> Element {
    let app_style = r#"
        html, body {
            background-color: #a0a0a0
    }"#;

    let page_signal = use_signal(|| AppPages::Home);

    rsx! {
        style { "{app_style}" }
        div {
            AppNavBar { page_signal: page_signal }
            AppMainPage { page_signal: page_signal }
        }
    }
}
