use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::Route;

#[component]
pub fn ModernHome() -> Element {
    // Stats for the dashboard - in a real implementation,
    // these would be fetched from your API
    let media_count = use_signal(|| 0);
    let collections_count = use_signal(|| 0);
    let libraries_count = use_signal(|| 0);

    let stats_loaded = use_signal(|| false);

    use_future(move || {
        to_owned![
            media_count,
            collections_count,
            libraries_count,
            stats_loaded
        ];
        async move {
            // Simulate an API call
            // In a real implementation, you would fetch real data
            media_count.set(3752);
            collections_count.set(48);
            libraries_count.set(5);
            stats_loaded.set(true);
        }
    });

    rsx! {
        div { class: "home-container",
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
                                    if media_count() > 0 {
                                        "{media_count()}"
                                    } else {
                                        div {
                                            class: "skeleton",
                                            style: "width: 80px; height: 32px;",
                                        }
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
                                    if collections_count() > 0 {
                                        "{collections_count()}"
                                    } else {
                                        div {
                                            class: "skeleton",
                                            style: "width: 80px; height: 32px;",
                                        }
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
                                    if libraries_count() > 0 {
                                        "{libraries_count()}"
                                    } else {
                                        div {
                                            class: "skeleton",
                                            style: "width: 80px; height: 32px;",
                                        }
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
                        button { class: "quick-action-card", onclick: move |_| {},
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
