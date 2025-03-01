use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::Route;

mod search;
pub use search::GallerySearch;

mod detail;
pub use detail::GalleryDetail;

const MEDIA_SEARCH_KEY: &str = "media_search";

// since we can't use a query path in a nested route, we instead use this key
// to keep track of which album we were browsing upon navigating here
pub const GALLERY_ALBUM_KEY: &str = "gallery_album";

#[component]
pub fn Gallery() -> Element {
    rsx! {
        Outlet::<Route> {}
    }
}
