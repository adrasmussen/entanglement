use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::Route;

// whether a nav link's page is the current one or an ancestor of it.  a search route carries
// its ?q= in the route, so two routes to the same page are only equal without the query
fn is_active(current: &Route, target: &Route) -> bool {
    let path = |r: &Route| {
        let s = r.to_string();
        let s = s.split(['?', '#']).next().unwrap_or_default();
        s.trim_end_matches('/').to_string()
    };

    current.is_child_of(target) || path(current) == path(target)
}

#[derive(Clone, PartialEq, Props)]
struct NavBarButtonProps {
    name: String,
    target: Route,
}

#[component]
fn NavBarButton(props: NavBarButtonProps) -> Element {
    let name = props.name;
    let target = props.target;

    let current_path: Route = use_route();
    rsx! {
        Link {
            class: if is_active(&current_path, &target) { "nav-link active" } else { "nav-link" },
            to: target,
            "{name}"
        }
    }
}

#[component]
fn NavBarInner() -> Element {
    rsx! {
        header { class: "app-header",
            div { class: "nav-container",
                div { class: "logo",
                    Link {
                        to: Route::ModernHome {},
                        style: "display: flex; align-items: center;",
                        img {
                            src: "/entanglement/app/assets/header.svg",
                            alt: "Entanglement",
                            style: "height: 32px; margin-right: 8px;",
                        }
                        span { style: "font-weight: 600; font-size: 1.25rem;", "Entanglement" }
                    }
                }

                nav { class: "nav-links",
                    NavBarButton {
                        name: "Gallery".to_owned(),
                        target: Route::GallerySearch { q: String::new() },
                    }
                    NavBarButton {
                        name: "Collections".to_owned(),
                        target: Route::CollectionSearch { q: String::new() },
                    }
                    NavBarButton {
                        name: "Libraries".to_owned(),
                        target: Route::LibrarySearch { q: String::new() },
                    }
                }
            }
        }
    }
}

#[component]
pub fn NavBar() -> Element {
    rsx! {
        NavBarInner {}
        Outlet::<Route> {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::query_ui::q_param;

    #[test]
    fn a_search_in_the_url_keeps_the_nav_link_active() {
        let gallery = Route::GallerySearch { q: String::new() };

        for q in ["", "tag:beach", "a b&c"] {
            let current = Route::GallerySearch { q: q_param(q) };
            assert!(is_active(&current, &gallery), "{q}");
            assert!(!is_active(
                &current,
                &Route::CollectionSearch { q: String::new() }
            ));
            assert!(!is_active(
                &current,
                &Route::LibrarySearch { q: String::new() }
            ));
        }

        // a page below the section, and the section from the detail page of another
        let detail = Route::CollectionDetail {
            collection_uuid: "x".into(),
            q: q_param("t"),
        };
        assert!(is_active(
            &detail,
            &Route::CollectionSearch { q: String::new() }
        ));
        assert!(!is_active(&detail, &gallery));
    }
}
