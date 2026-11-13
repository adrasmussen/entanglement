use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::{
    Route,
    components::modal::{MODAL_STACK, Modal, ModalBox},
};
use api::{
    collection::{SearchCollectionsReq, search_collections},
    library::{SearchLibrariesReq, search_libraries},
    media::{SearchMediaReq, search_media},
    search::SearchOptions,
};

#[component]
pub fn ModernHome() -> Element {
    // re-run on modal close, so creating a collection refreshes the count below
    let update_signal = use_signal(|| ());

    // dashboard stats, pulled from the same search endpoints the gallery/
    // collection/library pages use, with an empty/default filter to match
    // everything -- see SearchFilter's Default impl
    let media_count = use_resource(|| async move {
        search_media(&SearchMediaReq {
            opts: SearchOptions::default(),
        })
        .await
        .map(|resp| resp.media.len())
    });

    let collections_count = use_resource(move || async move {
        update_signal();

        search_collections(&SearchCollectionsReq {
            filter: String::new(),
        })
        .await
        .map(|resp| resp.collections.len())
    });

    let libraries_count = use_resource(|| async move {
        search_libraries(&SearchLibrariesReq {
            filter: String::new(),
        })
        .await
        .map(|resp| resp.libraries.len())
    });

    rsx! {
        div { class: "home-container",
            ModalBox { update_signal }

            // Hero section
            section { class: "hero",
                div { class: "container",
                    div { class: "hero-content",
                        h1 { class: "hero-title", "Entanglement" }
                        p { class: "hero-subtitle",
                            "Your personal media organization and gallery system"
                        }
                        div { class: "hero-actions",
                            Link {
                                to: Route::GallerySearch {},
                                class: "btn btn-primary btn-lg",
                                "Browse Gallery"
                            }
                            Link {
                                to: Route::CollectionSearch {},
                                class: "btn btn-secondary btn-lg",
                                "View Collections"
                            }
                        }
                    }
                }
            }

            // Stats section
            section { class: "stats-section",
                div { class: "container",
                    div { class: "stats-grid",
                        // Media stat card
                        div { class: "stat-card",
                            div { class: "stat-icon media-icon" }
                            div { class: "stat-content",
                                h3 { class: "stat-value",
                                    match &*media_count.read() {
                                        Some(Ok(count)) => rsx! { "{count}" },
                                        Some(Err(_)) => rsx! { "—" },
                                        None => rsx! {
                                            div {
                                                class: "skeleton",
                                                style: "width: 80px; height: 32px;",
                                            }
                                        },
                                    }
                                }
                                p { class: "stat-label", "Media Items" }
                            }
                            Link {
                                to: Route::GallerySearch {},
                                class: "stat-action",
                                "Browse All"
                            }
                        }

                        // Collections stat card
                        div { class: "stat-card",
                            div { class: "stat-icon collection-icon" }
                            div { class: "stat-content",
                                h3 { class: "stat-value",
                                    match &*collections_count.read() {
                                        Some(Ok(count)) => rsx! { "{count}" },
                                        Some(Err(_)) => rsx! { "—" },
                                        None => rsx! {
                                            div {
                                                class: "skeleton",
                                                style: "width: 80px; height: 32px;",
                                            }
                                        },
                                    }
                                }
                                p { class: "stat-label", "Collections" }
                            }
                            Link {
                                to: Route::CollectionSearch {},
                                class: "stat-action",
                                "View All"
                            }
                        }

                        // Libraries stat card
                        div { class: "stat-card",
                            div { class: "stat-icon library-icon" }
                            div { class: "stat-content",
                                h3 { class: "stat-value",
                                    match &*libraries_count.read() {
                                        Some(Ok(count)) => rsx! { "{count}" },
                                        Some(Err(_)) => rsx! { "—" },
                                        None => rsx! {
                                            div {
                                                class: "skeleton",
                                                style: "width: 80px; height: 32px;",
                                            }
                                        },
                                    }
                                }
                                p { class: "stat-label", "Libraries" }
                            }
                            Link {
                                to: Route::LibrarySearch {},
                                class: "stat-action",
                                "Manage"
                            }
                        }
                    }
                }
            }

            // Features section
            section { class: "features-section",
                div { class: "container",
                    h2 { class: "section-title", "Features" }

                    div { class: "features-grid",
                        // Feature 1
                        div { class: "feature-card",
                            div { class: "feature-icon organize-icon" }
                            h3 { class: "feature-title", "Organize Your Media" }
                            p { class: "feature-desc",
                                "Sort, tag, and categorize your photos and videos into collections and collections."
                            }
                        }

                        // Feature 2
                        div { class: "feature-card",
                            div { class: "feature-icon search-icon" }
                            h3 { class: "feature-title", "Powerful Search" }
                            p { class: "feature-desc",
                                "Find exactly what you're looking for with text-based search across metadata."
                            }
                        }

                        // Feature 3
                        div { class: "feature-card",
                            div { class: "feature-icon secure-icon" }
                            h3 { class: "feature-title", "Personal & Secure" }
                            p { class: "feature-desc",
                                "Your media stays on your own hardware, giving you complete control and privacy."
                            }
                        }

                        // Feature 4
                        div { class: "feature-card",
                            div { class: "feature-icon responsive-icon" }
                            h3 { class: "feature-title", "Modern Interface" }
                            p { class: "feature-desc",
                                "Enjoy a clean, responsive design that works on any device or screen size."
                            }
                        }
                    }
                }
            }

            // Quick actions section
            section { class: "quick-actions",
                div { class: "container",
                    h2 { class: "section-title", "Quick Actions" }

                    div { class: "actions-grid",
                        Link {
                            to: Route::GallerySearch {},
                            class: "quick-action-card",
                            div { class: "quick-action-icon browse-icon" }
                            span { "Browse Gallery" }
                        }
                        Link {
                            to: Route::CollectionSearch {},
                            class: "quick-action-card",
                            div { class: "quick-action-icon collections-icon" }
                            span { "View Collections" }
                        }
                        button {
                            class: "quick-action-card",
                            onclick: move |_| {
                                MODAL_STACK.with_mut(|v| v.push(Modal::CreateCollection));
                            },
                            div { class: "quick-action-icon new-collection-icon" }
                            span { "Create Collection" }
                        }
                        Link {
                            to: Route::LibrarySearch {},
                            class: "quick-action-card",
                            div { class: "quick-action-icon libraries-icon" }
                            span { "Manage Libraries" }
                        }
                    }
                }
            }

            // Footer
            footer { class: "home-footer",
                div { class: "container",
                    p { "Entanglement • Personal Media Organization System" }
                }
            }
        }
    }
}
