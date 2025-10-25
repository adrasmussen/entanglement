use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::Route;

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
            class: if current_path.is_child_of(&target) || current_path == (target) { "nav-link active" } else { "nav-link" },
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
                    Link { to: Route::ModernHome {}, style: "display: flex; align-items: center;",
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
                        target: Route::GallerySearch {},
                    }
                    NavBarButton {
                        name: "Collections".to_owned(),
                        target: Route::CollectionSearch {},
                    }
                    NavBarButton {
                        name: "Libraries".to_owned(),
                        target: Route::LibrarySearch {},
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
