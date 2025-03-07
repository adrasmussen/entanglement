// webapp/src/library/mod.rs

use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::Route;

mod table;

mod detail;
pub use detail::LibraryDetail;

mod search;
pub use search::LibrarySearch;

pub const LIBRARY_SEARCH_KEY: &str = "library_search";
pub const MEDIA_SEARCH_KEY: &str = "media_in_library_search";

#[component]
pub fn Libraries() -> Element {
    rsx! {
        Outlet::<Route> {}
    }
}
