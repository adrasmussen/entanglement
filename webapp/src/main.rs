#![allow(non_snake_case)]
use dioxus::prelude::*;
use dioxus_router::prelude::*;

use tracing::Level;

mod common;

mod components;
use components::navigation::NavBar;

mod home;
use home::ModernHome;

mod gallery;
use gallery::{Gallery, GalleryDetail, GallerySearch};

mod collection;
use collection::{CollectionDetail, CollectionSearch, Collections};

mod library;
use library::{Libraries, LibraryDetail, LibrarySearch};

fn main() {
    dioxus_logger::init(Level::DEBUG).expect("failed to init logger");
    launch(App);
}

// Probably the easiest way to make the gallery view useful is to
// have an optional ?collection=XXX,library=XXX when jumping back to the
// GalleryDetail for particular media
//
// this enables menus to say "came from this collection -- remove?" and
// possibly other things

#[derive(Clone, PartialEq, Routable)]
#[rustfmt::skip]
enum Route {
    #[layout(NavBar)]
        #[route("/")]
        ModernHome {},
        #[nest("/gallery")]
            #[layout(Gallery)]
                #[route("/?:q")]
                GallerySearch { q: String },
                #[route("/:media_uuid")]
                GalleryDetail { media_uuid: String },
            #[end_layout]
        #[end_nest]
        #[nest("/collections")]
            #[layout(Collections)]
                #[route("/?:q")]
                CollectionSearch { q: String },
                #[route("/:collection_uuid?:q")]
                CollectionDetail { collection_uuid: String, q: String },
            #[end_layout]
        #[end_nest]
        #[nest("/library")]
            #[layout(Libraries)]
                #[route("/?:q")]
                LibrarySearch { q: String },
                #[route("/:library_uuid?:q")]
                LibraryDetail { library_uuid: String, q: String },
}

#[component]
pub fn App() -> Element {
    rsx! {
        style { "{common::style::MODERN_STYLES}" }
        style { "{common::style::HOME_STYLES}" }
        Router::<Route> { config: RouterConfig::default }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::query_ui::{q_from_param, q_param};

    const UUID: &str = "00000000-0000-0000-0000-000000000001";

    // the page-side view of a route's ?q=: what its component gets as `q`, decoded again
    fn page_q(route: &Route) -> String {
        match route {
            Route::GallerySearch { q }
            | Route::CollectionSearch { q }
            | Route::LibrarySearch { q }
            | Route::CollectionDetail { q, .. }
            | Route::LibraryDetail { q, .. } => q_from_param(q),
            other => panic!("not a search route: {other}"),
        }
    }

    fn routes(text: &str) -> Vec<Route> {
        let q = q_param(text);
        vec![
            Route::GallerySearch { q: q.clone() },
            Route::CollectionSearch { q: q.clone() },
            Route::LibrarySearch { q: q.clone() },
            Route::CollectionDetail {
                collection_uuid: UUID.into(),
                q: q.clone(),
            },
            Route::LibraryDetail {
                library_uuid: UUID.into(),
                q,
            },
        ]
    }

    #[test]
    fn search_text_survives_the_url() {
        for text in [
            "",
            "beach",
            "tag:ct-mod7 date:2022",
            "two  spaces\tand a tab",
            r#"description:"a big wave" "quoted words""#,
            r#"description:"say \"hi\" \\""#,
            "50% off 100%25 %26 %2526 %",
            "a&b c=d e?f g#h i/j k+l",
            "date:>=2020-01 date:<2021 date:2020..2021",
            "\u{e9}t\u{e9} \u{4e2d}\u{6587} \u{1f600}",
            "&&&===???###",
        ] {
            for route in routes(text) {
                let url = route.to_string();
                let parsed: Route = url
                    .parse()
                    .unwrap_or_else(|e| panic!("{url} does not parse: {e:?}"));
                assert!(parsed == route, "{url}");
                assert_eq!(page_q(&parsed), text, "{url}");
            }
        }
    }

    #[test]
    fn the_url_reads_as_a_q_parameter() {
        let r = Route::GallerySearch {
            q: q_param("tag:beach"),
        };
        assert_eq!(r.to_string(), "/gallery/?q=tag%3Abeach");

        let r = Route::CollectionDetail {
            collection_uuid: UUID.into(),
            q: q_param("a b&c"),
        };
        assert_eq!(r.to_string(), format!("/collections/{UUID}?q=a%20b%2526c"));

        // what omnisearch and hand-written links will use, unescaped for the ordinary characters
        let parsed: Route = "/gallery?q=tag:beach%20date:2022".parse().unwrap();
        assert_eq!(page_q(&parsed), "tag:beach date:2022");
        let parsed: Route = format!("/library/{UUID}?q=path%3Aphotos").parse().unwrap();
        assert_eq!(page_q(&parsed), "path:photos");

        // no q at all is an empty search, which falls back to the stored one
        let parsed: Route = "/gallery".parse().unwrap();
        assert_eq!(page_q(&parsed), "");
        let parsed: Route = "/gallery/?q=".parse().unwrap();
        assert_eq!(page_q(&parsed), "");
    }
}
