use dioxus::prelude::*;

// ideally, this exposes a folder-by-folder view of the user's library

#[component]
pub fn Library() -> Element {
    rsx! {
        span { "library" }
    }
}
