use dioxus::prelude::*;

use crate::{
    common::storage::try_local_storage,
    components::{media_card::MediaCard, modal::ModalBox, search_bar::SearchBar},
    gallery::MEDIA_SEARCH_KEY,
};
use api::{media::*, search::SearchFilter};

#[component]
pub fn GallerySearch() -> Element {
    let update_signal = use_signal(|| ());

    let mut advanced_expanded = use_signal(|| false);
    let mut active_tab = use_signal(|| "text");

    let mut media_search_signal = use_signal::<String>(|| try_local_storage(MEDIA_SEARCH_KEY));

    let media_future = use_resource(move || async move {
        let filter = media_search_signal()
            .split_whitespace()
            .map(|s| s.to_owned())
            .collect();

        search_media(&SearchMediaReq {
            filter: SearchFilter::SubstringAny { filter },
        })
        .await
    });

    let action_button = rsx! {
        button {
            class: "btn btn-secondary",
            onclick: move |_| {
                advanced_expanded.set(!advanced_expanded());
            },
            if advanced_expanded() {
                "Hide Advanced"
            } else {
                "Advanced"
            }
        }
    };

    rsx! {
        div { class: "container",
            ModalBox { update_signal }

            div { class: "page-header", style: "margin-bottom: var(--space-4);",
                h1 { class: "section-title", "Photo Gallery" }
                p { "Browse and search all accessible media" }
            }

            SearchBar {
                search_signal: media_search_signal,
                storage_key: MEDIA_SEARCH_KEY,
                placeholder: "Search by date or description...",
                status: match &*media_future.read() {
                    Some(Ok(resp)) => format!("Found {} results", resp.media.len()),
                    Some(Err(_)) => String::from("Error searching media"),
                    None => String::from("Loading..."),
                },
                action_button,
            }

            {
                if advanced_expanded() {
                    rsx! {
                        div {
                            class: "advanced-search-options",
                            style: "
                                                        margin-top: -16px;
                                                        margin-bottom: var(--space-6);
                                                        padding: var(--space-4);
                                                        background-color: var(--neutral-50);
                                                        border-radius: 0 0 var(--radius-lg) var(--radius-lg);
                                                        box-shadow: var(--shadow-sm);
                                                        border-top: 1px solid var(--neutral-200);
                                                        animation: slide-down 0.2s ease-out;
                                                    ",
                            h3 { style: "margin-bottom: var(--space-3); font-size: 1rem;", "Advanced Search Options" }
                        
                            // Tabs navigation
                            div {
                                class: "tabs-navigation",
                                style: "
                                                            display: flex;
                                                            border-bottom: 1px solid var(--neutral-200);
                                                            margin-bottom: var(--space-4);
                                                        ",
                        
                                button {
                                    class: if active_tab() == "text" { "tab-button active" } else { "tab-button" },
                                    style: "
                                                                        padding: var(--space-2) var(--space-4);
                                                                        background: none;
                                                                        border: none;
                                                                        border-bottom: 3px solid transparent;
                                                                        cursor: pointer;
                                                                        font-weight: 500;
                                                                        color: var(--text-secondary);
                                                                        transition: all var(--transition-fast) var(--easing-standard);
                                                                        margin-right: var(--space-2);
                                                                        "
                                        .to_string()
                                        + if active_tab() == "text" {
                                            "color: var(--primary); border-bottom-color: var(--primary);"
                                        } else {
                                            ""
                                        },
                                    onclick: move |_| active_tab.set("text"),
                        
                                    "Text Search"
                                }
                        
                                button {
                                    class: if active_tab() == "date" { "tab-button active" } else { "tab-button" },
                                    style: "
                                                                        padding: var(--space-2) var(--space-4);
                                                                        background: none;
                                                                        border: none;
                                                                        border-bottom: 3px solid transparent;
                                                                        cursor: pointer;
                                                                        font-weight: 500;
                                                                        color: var(--text-secondary);
                                                                        transition: all var(--transition-fast) var(--easing-standard);
                                                                        margin-right: var(--space-2);
                                                                        "
                                        .to_string()
                                        + if active_tab() == "date" {
                                            "color: var(--primary); border-bottom-color: var(--primary);"
                                        } else {
                                            ""
                                        },
                                    onclick: move |_| active_tab.set("date"),
                        
                                    "Date Filters"
                                }
                        
                                button {
                                    class: if active_tab() == "metadata" { "tab-button active" } else { "tab-button" },
                                    style: "
                                                                        padding: var(--space-2) var(--space-4);
                                                                        background: none;
                                                                        border: none;
                                                                        border-bottom: 3px solid transparent;
                                                                        cursor: pointer;
                                                                        font-weight: 500;
                                                                        color: var(--text-secondary);
                                                                        transition: all var(--transition-fast) var(--easing-standard);
                                                                        margin-right: var(--space-2);
                                                                        "
                                        .to_string()
                                        + if active_tab() == "metadata" {
                                            "color: var(--primary); border-bottom-color: var(--primary);"
                                        } else {
                                            ""
                                        },
                                    onclick: move |_| active_tab.set("metadata"),
                        
                                    "Metadata"
                                }
                        
                                button {
                                    class: if active_tab() == "similar" { "tab-button active" } else { "tab-button" },
                                    style: "
                                                                        padding: var(--space-2) var(--space-4);
                                                                        background: none;
                                                                        border: none;
                                                                        border-bottom: 3px solid transparent;
                                                                        cursor: pointer;
                                                                        font-weight: 500;
                                                                        color: var(--text-secondary);
                                                                        transition: all var(--transition-fast) var(--easing-standard);
                                                                        "
                                        .to_string()
                                        + if active_tab() == "similar" {
                                            "color: var(--primary); border-bottom-color: var(--primary);"
                                        } else {
                                            ""
                                        },
                                    onclick: move |_| active_tab.set("similar"),
                        
                                    "Similar Media"
                                }
                            }
                        
                            // Tab content
                            div { class: "tab-content", style: "min-height: 200px;",
                        
                                match active_tab() {
                                    "text" => rsx! {
                                        div { class: "text-search-options",
                                            div { class: "form-group",
                                                label { class: "form-label", "Search Terms" }
                                                input {
                                                    class: "form-input",
                                                    placeholder: "Enter keywords separated by spaces...",
                                                    value: "{media_search_signal()}",
                                                    oninput: move |evt| media_search_signal.set(evt.value().clone()),
                                                }
                                                div {
                                                    class: "form-help",
                                                    style: "font-size: 0.875rem; color: var(--text-tertiary); margin-top: var(--space-1);",
                                                    "Search in file names, descriptions, and tags"
                                                }
                                            }
                                        
                                            div { style: "display: flex; gap: var(--space-4); margin-top: var(--space-4);",
                                                div { class: "form-group", style: "flex: 1;",
                                                    label { class: "form-label", "Search Mode" }
                                                    select { class: "form-select",
                                                        option { value: "all", selected: true, "Match All Words" }
                                                        option { value: "any", "Match Any Word" }
                                                        option { value: "exact", "Exact Phrase" }
                                                    }
                                                }
                                        
                                                div { class: "form-group", style: "flex: 1;",
                                                    label { class: "form-label", "Case Sensitive" }
                                                    div { style: "display: flex; align-items: center; height: 38px;", // Match height of select
                                                        input {
                                                            r#type: "checkbox",
                                                            id: "case-sensitive",
                                                            style: "margin-right: var(--space-2);",
                                                        }
                                                        label { r#for: "case-sensitive", "Enable case sensitivity" }
                                                    }
                                                }
                                            }
                                        }
                                    },
                                    "date" => rsx! {
                                        div { class: "date-filter-options",
                                            div { style: "display: flex; gap: var(--space-4);",
                                                div { class: "form-group", style: "flex: 1;",
                                                    label { class: "form-label", "From Date" }
                                                    input { class: "form-input", r#type: "date" }
                                                }
                                        
                                                div { class: "form-group", style: "flex: 1;",
                                                    label { class: "form-label", "To Date" }
                                                    input { class: "form-input", r#type: "date" }
                                                }
                                            }
                                        
                                            div { class: "form-group", style: "margin-top: var(--space-4);",
                                                label { class: "form-label", "Quick Date Ranges" }
                                                div { style: "display: flex; flex-wrap: wrap; gap: var(--space-2); margin-top: var(--space-2);",
                                                    button { class: "btn btn-sm btn-secondary", "Today" }
                                                    button { class: "btn btn-sm btn-secondary", "This Week" }
                                                    button { class: "btn btn-sm btn-secondary", "This Month" }
                                                    button { class: "btn btn-sm btn-secondary", "This Year" }
                                                    button { class: "btn btn-sm btn-secondary", "Last 30 Days" }
                                                    button { class: "btn btn-sm btn-secondary", "Last 90 Days" }
                                                }
                                            }
                                        }
                                    },
                                    "metadata" => rsx! {
                                        div { class: "metadata-filter-options",
                                            div { style: "display: flex; gap: var(--space-4);",
                                                div { class: "form-group", style: "flex: 1;",
                                                    label { class: "form-label", "Media Type" }
                                                    select { class: "form-select",
                                                        option { value: "all", selected: true, "All Types" }
                                                        option { value: "image", "Images Only" }
                                                        option { value: "video", "Videos Only" }
                                                        option { value: "audio", "Audio Only" }
                                                    }
                                                }
                                        
                                                div { class: "form-group", style: "flex: 1;",
                                                    label { class: "form-label", "Tags" }
                                                    input { class: "form-input", placeholder: "Enter tags separated by |" }
                                                }
                                            }
                                        
                                            div { class: "form-group", style: "margin-top: var(--space-4);",
                                                label { class: "form-label", "Show Hidden Files" }
                                                div { style: "display: flex; align-items: center;",
                                                    input {
                                                        r#type: "checkbox",
                                                        id: "show-hidden",
                                                        style: "margin-right: var(--space-2);",
                                                    }
                                                    label { r#for: "show-hidden", "Include hidden media in search results" }
                                                }
                                            }
                                        }
                                    },
                                    "similar" => rsx! {
                                        div { class: "similar-media-options",
                                            p { style: "margin-bottom: var(--space-4); color: var(--text-secondary);",
                                                "Find media that are similar to a specific item."
                                            }
                                        
                                            div { class: "form-group",
                                                label { class: "form-label", "Media UUID" }
                                                div { style: "display: flex; gap: var(--space-2);",
                                                    input {
                                                        class: "form-input",
                                                        style: "flex-grow: 1;",
                                                        placeholder: "Enter media UUID to find similar items",
                                                    }
                                                    button { class: "btn btn-secondary", "Browse..." }
                                                }
                                            }
                                        
                                            div { class: "form-group", style: "margin-top: var(--space-4);",
                                                label { class: "form-label", "Similarity Threshold" }
                                                select { class: "form-select",
                                                    option { value: "64", "Very Similar" }
                                                    option { value: "84", selected: true, "Similar" }
                                                    option { value: "106", "Somewhat Similar" }
                                                    option { value: "128", "Broadly Similar" }
                                                }
                                            }
                                        }
                                    },
                                    _ => rsx! {
                                        div {}
                                    },
                                }
                            }
                        
                            // Action buttons
                            div {
                                class: "advanced-search-actions",
                                style: "
                                                            display: flex;
                                                            justify-content: flex-end;
                                                            margin-top: var(--space-4);
                                                            padding-top: var(--space-4);
                                                            border-top: 1px solid var(--neutral-200);
                                                        ",
                        
                                button {
                                    class: "btn btn-secondary",
                                    style: "margin-right: var(--space-2);",
                                    "Reset Filters"
                                }
                        
                                button { class: "btn btn-primary", "Apply Filters" }
                            }
                        }
                    }
                } else {
                    rsx! {}
                }
            }

            match &*media_future.read() {
                Some(Ok(resp)) => {
                    rsx! {
                        if resp.media.is_empty() {
                            div { class: "empty-state",
                                p { "No media found matching your search criteria." }
                            }
                        } else {
                            div { class: "media-grid",
                                for media_uuid in resp.media.iter() {
                                    MediaCard { key: "{media_uuid}", media_uuid: *media_uuid }
                                }
                            }
                        }
                    }
                }
                Some(Err(err)) => rsx! {
                    div { class: "error-state",
                        p { "Error: {err}" }
                    }
                },
                None => rsx! {
                    div { class: "loading-state media-grid",
                        for _ in 0..8 {
                            div { class: "skeleton-card",
                                div { class: "skeleton", style: "height: 200px;" }
                                div {
                                    class: "skeleton",
                                    style: "height: 24px; width: 40%; margin-top: 12px;",
                                }
                                div {
                                    class: "skeleton",
                                    style: "height: 18px; width: 80%; margin-top: 8px;",
                                }
                            }
                        }
                    }
                },
            }
        }
    }
}
