use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::Route;

mod grid;

mod detail;
pub use detail::GalleryDetail;

mod search;
pub use search::GallerySearch;

const MEDIA_SEARCH_KEY: &str = "media_search";

#[component]
pub fn Gallery() -> Element {
    rsx! {
        Outlet::<Route> {}
    }
}
