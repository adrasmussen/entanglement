use dioxus::prelude::*;

#[component]
pub fn Status() -> Element {
    rsx! {
        span { "status" }
    }
}