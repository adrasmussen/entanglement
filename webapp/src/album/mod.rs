use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::Route;

pub mod card;
pub mod grid;

mod detail;
pub use detail::AlbumDetail;

mod search;
pub use search::AlbumSearch;

const ALBUM_SEARCH_KEY: &str = "album_search";
const MEDIA_SEARCH_KEY: &str = "media_in_album_search";

#[component]
pub fn Albums() -> Element {
    rsx! {
        Outlet::<Route> {}
    }
}
