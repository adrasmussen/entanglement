use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::Route;

#[component]
pub fn ModernNavBar() -> Element {
    let current_path: Route = use_route();

    rsx! {
        header { class: "app-header",
            div { class: "nav-container",
                // Logo area
                div { class: "logo",
                    Link { to: Route::ModernHome {}, class: "flex items-center",
                        img {
                            src: "/entanglement/app/assets/header.svg",
                            alt: "Entanglement",
                            style: "height: 32px; margin-right: 8px;",
                        }
                        span { style: "font-weight: 600; font-size: 1.25rem;", "Entanglement" }
                    }
                }

                // Navigation links
                nav { class: "nav-links",
                    Link {
                        to: Route::GallerySearch {},
                        class: if current_path.is_child_of(&Route::GallerySearch {})
    || current_path == (Route::GallerySearch {}) { "nav-link active" } else { "nav-link" },
                        "Gallery"
                    }
                    Link {
                        to: Route::CollectionSearch {},
                        class: if current_path.is_child_of(&Route::CollectionSearch {})
    || current_path == (Route::CollectionSearch {}) { "nav-link active" } else { "nav-link" },
                        "Collections"
                    }
                    Link {
                        to: Route::LibrarySearch {},
                        class: if current_path.is_child_of(&Route::LibrarySearch {})
    || current_path == (Route::LibrarySearch {}) { "nav-link active" } else { "nav-link" },
                        "Libraries"
                    }
                }
            }
        }
    }
}
