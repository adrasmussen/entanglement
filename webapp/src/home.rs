use dioxus::prelude::*;
use dioxus_router::prelude::*;
use tracing::error;

use crate::{
    Route,
    components::modal::{MODAL_STACK, Modal, ModalBox},
};
use api::{
    WebError,
    search::{CountScope, GetCountsReq, get_counts},
};

// the dashboard cards, in the order they are requested
const STAT_SCOPES: [CountScope; 3] = [
    CountScope::Media,
    CountScope::Collections,
    CountScope::Libraries,
];

#[component]
pub fn ModernHome() -> Element {
    // re-run on modal close, so creating a collection refreshes the counts below
    let update_signal = use_signal(|| ());

    // dashboard stats: one request for all three, which counts exactly what the gallery,
    // collection and library pages would list
    let counts = use_resource(move || async move {
        update_signal();

        let resp = get_counts(&GetCountsReq {
            scopes: STAT_SCOPES.to_vec(),
        })
        .await?;

        let count_of = |scope: &CountScope| {
            resp.counts
                .iter()
                .find(|c| &c.scope == scope)
                .map(|c| c.count)
                .ok_or_else(|| {
                    let err = WebError::msg(format!("no count came back for {scope:?}"));
                    error!("{err}");
                    err
                })
        };

        Ok::<_, WebError>((
            count_of(&STAT_SCOPES[0])?,
            count_of(&STAT_SCOPES[1])?,
            count_of(&STAT_SCOPES[2])?,
        ))
    });

    // one value per card; the same loading and error rendering for all of them
    let stat = |pick: fn(&(u64, u64, u64)) -> u64| match &*counts.read() {
        Some(Ok(all)) => rsx! { "{pick(all)}" },
        Some(Err(_)) => rsx! { "—" },
        None => rsx! {
            div { class: "skeleton", style: "width: 80px; height: 32px;" }
        },
    };

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
                                to: Route::GallerySearch { q: String::new() },
                                class: "btn btn-primary btn-lg",
                                "Browse Gallery"
                            }
                            Link {
                                to: Route::CollectionSearch { q: String::new() },
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
                                    {stat(|c| c.0)}
                                }
                                p { class: "stat-label", "Media Items" }
                            }
                            Link {
                                to: Route::GallerySearch { q: String::new() },
                                class: "stat-action",
                                "Browse All"
                            }
                        }

                        // Collections stat card
                        div { class: "stat-card",
                            div { class: "stat-icon collection-icon" }
                            div { class: "stat-content",
                                h3 { class: "stat-value",
                                    {stat(|c| c.1)}
                                }
                                p { class: "stat-label", "Collections" }
                            }
                            Link {
                                to: Route::CollectionSearch { q: String::new() },
                                class: "stat-action",
                                "View All"
                            }
                        }

                        // Libraries stat card
                        div { class: "stat-card",
                            div { class: "stat-icon library-icon" }
                            div { class: "stat-content",
                                h3 { class: "stat-value",
                                    {stat(|c| c.2)}
                                }
                                p { class: "stat-label", "Libraries" }
                            }
                            Link {
                                to: Route::LibrarySearch { q: String::new() },
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
                            to: Route::GallerySearch { q: String::new() },
                            class: "quick-action-card",
                            div { class: "quick-action-icon browse-icon" }
                            span { "Browse Gallery" }
                        }
                        Link {
                            to: Route::CollectionSearch { q: String::new() },
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
                            to: Route::LibrarySearch { q: String::new() },
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
