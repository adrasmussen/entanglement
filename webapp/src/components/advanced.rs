use std::collections::{HashMap, HashSet};

use dioxus::prelude::*;

use crate::components::modal::{MODAL_STACK, Modal};
use api::{WebError, media::MediaUuid};

// TODO -- use show_signal for consistent cleanup... or move to a button?
#[derive(Clone, PartialEq, Props)]
pub struct AdvancedTabsProps {
    show_signal: Signal<bool>,
    tabs: HashMap<String, Element>,
}

#[component]
pub fn AdvancedTabs(props: AdvancedTabsProps) -> Element {
    let show_signal = props.show_signal;
    let tabs = props.tabs;

    if !show_signal() {
        return rsx! {};
    }

    let mut tab_names = tabs.keys().cloned().collect::<Vec<String>>();

    tab_names.sort();

    let default_tab = match tab_names.first() {
        Some(v) => v.clone(),
        None => return rsx! {},
    };

    let mut tab_signal = use_signal(|| default_tab);

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
            h3 { style: "margin-bottom: var(--space-3); font-size: 1rem;", "Advanced Options" }

            div {
                class: "tabs-navigation",
                style: "display: flex; border-bottom: 1px solid var(--neutral-200); margin-bottom: var(--space-4);",

                for name in tab_names {

                    button {
                        class: if *tab_signal.read() == *name { "tab-button active" } else { "tab-button" },
                        style: "padding: var(--space-2) var(--space-4); background: none; border: none; border-bottom: 3px solid transparent; cursor: pointer;font-weight: 500; color: var(--text-secondary); transition: all var(--transition-fast) var(--easing-standard); margin-right: var(--space-2);"
                            .to_string()
                            + if *tab_signal.read() == *name {
                                "color: var(--primary); border-bottom-color: var(--primary);"
                            } else {
                                ""
                            },
                        onclick: move |_| tab_signal.set(name.clone()),

                        "{name}"
                    }
                }
            }

            ErrorBoundary {
                handle_error: |error: ErrorContext| {
                    rsx! {
                        if let Some(error_ui) = error.show() {
                            {error_ui}
                        } else {
                            div { "Tab encountered an error.  Check the logs or reach out the the administrators." }
                        }
                    }
                },

                {
                    tabs.get(&*tab_signal.read())
                        .ok_or_else(|| WebError::msg("unknown tab".to_owned()))?
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct AdvancedSearchTabProps {
    media_search_signal: Signal<String>,
}

#[component]
pub fn AdvancedSearchTab(props: AdvancedSearchTabProps) -> Element {
    let mut media_search_signal = props.media_search_signal;
    rsx! {
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
                    div { style: "display: flex; align-items: center; height: 38px;",
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
    }
}

#[derive(Clone, PartialEq)]
pub enum BulkEditMode {
    EditTags,
    AddToCollection,
    //RmFromCollection,
    //Hide,
}
#[derive(Clone, PartialEq, Props)]
struct BulkEditButtonProps {
    mode: BulkEditMode,
    bulk_edit_signal: Signal<Option<HashSet<MediaUuid>>>,
}

#[component]
fn BulkEditButton(props: BulkEditButtonProps) -> Element {
    let bulk_edit_signal = props.bulk_edit_signal;

    let (text, target) = match props.mode {
        BulkEditMode::EditTags => ("Edit Tags", Modal::BulkEditTags(bulk_edit_signal())),
        BulkEditMode::AddToCollection => (
            "Add to Collection",
            Modal::BulkAddToCollection(bulk_edit_signal()),
        ),
    };

    rsx! {
        button {
            class: "btn btn-primary",
            onclick: move |_| {
                MODAL_STACK.with_mut(|v| { v.push(target.clone()) });
            },
            "{text}"
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct BulkEditTabProps {
    bulk_edit_signal: Signal<Option<HashSet<MediaUuid>>>,
    media_uuids: Memo<Option<HashSet<MediaUuid>>>,
    modes: Vec<BulkEditMode>,
}

#[component]
pub fn BulkEditTab(props: BulkEditTabProps) -> Element {
    let mut bulk_edit_signal = props.bulk_edit_signal;
    let media_uuids = props.media_uuids;
    let modes = props.modes;

    rsx! {
        div { class: "bulk-edit-options",
            div {
                class: "bulk-edit-controls",
                style: "margin-bottom: var(--space-4);",
                div { class: "form-group",
                    div { style: "display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-4);",
                        {
                            match bulk_edit_signal() {
                                None => {
                                    rsx! {
                                        button {
                                            class: "btn btn-primary",
                                            onclick: move |_| {
                                                bulk_edit_signal.set(Some(HashSet::new()));
                                            },
                                            "Enable Bulk Edit Mode"
                                        }
                                    }
                                }
                                Some(_) => {
                                    rsx! {
                                        button {
                                            class: "btn btn-danger",
                                            onclick: move |_| {
                                                bulk_edit_signal.set(None);
                                            },
                                            "Disable Bulk Edit Mode"
                                        }
                                    }
                                }
                            }
                        }

                        if let Some(media_uuids) = bulk_edit_signal() {
                            span { style: "color: var(--text-secondary);",
                                "{media_uuids.len()} items selected"
                            }
                        }
                    }
                }
            }
        }

        if let Some(bulk_edit_uuids) = bulk_edit_signal() {
            div {
                class: "bulk-actions",
                style: "display: flex; gap: var(--space-3); margin-top: var(--space-4);",
                button {
                    class: "btn btn-secondary",
                    onclick: move |_| {
                        if let Some(media_uuids) = media_uuids() {
                            bulk_edit_signal
                                .set(Some(media_uuids.difference(&bulk_edit_uuids).copied().collect()));
                        }
                    },
                    "Invert Selection"
                }
                button {
                    class: "btn btn-secondary",
                    onclick: move |_| {
                        bulk_edit_signal.set(Some(HashSet::new()));
                    },
                    "Clear Selection"
                }
            }
            div {
                class: "bulk-actions",
                style: "display: flex; gap: var(--space-3); margin-top: var(--space-4);",


                for mode in modes {
                    BulkEditButton { bulk_edit_signal, mode }
                }

            }
            div {
                class: "bulk-edit-instructions",
                style: "margin-top: var(--space-4); padding: var(--space-3); background-color: var(--neutral-50); border-radius: var(--radius-md);",
                p { style: "margin: 0; color: var(--text-secondary);",
                    "Click on media items in the gallery to select them. Click again to deselect."
                }
                p { style: "margin-top: var(--space-2); color: var(--text-tertiary);",
                    "Selected items will be highlighted with a blue border."
                }
            }
        } else {
            div {
                class: "bulk-edit-disabled",
                style: "margin-top: var(--space-4); padding: var(--space-3); background-color: var(--neutral-50); border-radius: var(--radius-md);",
                p { style: "margin: 0; color: var(--text-secondary);",
                    "Enable bulk edit mode to modify many media at the same time."
                }
            }
        }
    }
}
