use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::Route;

pub mod card;
pub mod grid;

mod detail;
pub use detail::CollectionDetail;

mod search;
pub use search::CollectionSearch;

const COLLECTION_SEARCH_KEY: &str = "collection_search";
const MEDIA_SEARCH_KEY: &str = "media_in_collection_search";

#[component]
pub fn Collections() -> Element {
    rsx! {
        Outlet::<Route> {}
    }
}
