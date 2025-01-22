use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::Route;

mod grid;

mod detail;
pub use detail::GalleryDetail;

mod search;
pub use search::GallerySearch;

#[component]
pub fn Gallery() -> Element {
    rsx! {
        Outlet::<Route> {}
    }
}
