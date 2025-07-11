use std::collections::HashSet;

use dioxus::prelude::*;

use crate::components::modal::{MODAL_STACK, Modal};
use api::media::MediaUuid;

pub static BULK_EDIT: GlobalSignal<Option<HashSet<MediaUuid>>> = Signal::global(|| None);

#[derive(Clone, PartialEq)]
enum TabTarget {
    Search,
    BulkEditTags,
    BulkAddToCollection,
    //BulkHide,
    //BulkRmFromCollection,
}

// TODO -- have this take a list of tabs so that different search contexts
// have access to different advanced options (and so that the tabs can
// hold references to those contexts)
#[derive(Clone, PartialEq, Props)]
pub struct AdvancedContainerProps {
    media_search_signal: Signal<String>,
}

#[component]
pub fn AdvancedContainer(props: AdvancedContainerProps) -> Element {
    let media_search_signal = props.media_search_signal;

    let tab_signal: Signal<TabTarget> = use_signal(|| TabTarget::Search);

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
                AdvancedTab {
                    tab_signal,
                    target: TabTarget::Search,
                    text: "Search Options",
                }
                AdvancedTab {
                    tab_signal,
                    target: TabTarget::BulkEditTags,
                    text: "Bulk Edit Tags",
                }
                AdvancedTab {
                    tab_signal,
                    target: TabTarget::BulkAddToCollection,
                    text: "Bulk Add to Collection",
                }
            }

            AdvancedContent { tab_signal, media_search_signal }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
struct AdvancedTabProps {
    tab_signal: Signal<TabTarget>,
    target: TabTarget,
    text: String,
}

#[component]
fn AdvancedTab(props: AdvancedTabProps) -> Element {
    let mut tab_signal = props.tab_signal;
    let target = props.target;
    let text = props.text;

    rsx! {
        button {
            class: if *tab_signal.read() == target { "tab-button active" } else { "tab-button" },
            style: "padding: var(--space-2) var(--space-4); background: none; border: none; border-bottom: 3px solid transparent; cursor: pointer;font-weight: 500; color: var(--text-secondary); transition: all var(--transition-fast) var(--easing-standard); margin-right: var(--space-2);"
                .to_string()
                + if *tab_signal.read() == target {
                    "color: var(--primary); border-bottom-color: var(--primary);"
                } else {
                    ""
                },
            // the bulk-edit modes want to share context if the tabs change,
            // but generic tabs may want to reset their specialty signals
            onclick: move |_| tab_signal.set(target.clone()),

            "{text}"
        }
    }
}

#[derive(Clone, PartialEq, Props)]
struct AdvancedContentProps {
    tab_signal: Signal<TabTarget>,
    media_search_signal: Signal<String>,
}

#[component]
fn AdvancedContent(props: AdvancedContentProps) -> Element {
    let tab_signal = props.tab_signal;
    let mut media_search_signal = props.media_search_signal;

    rsx! {
        div { class: "tab-content", style: "min-height: 200px;",

            match tab_signal() {
                TabTarget::Search => {
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
                TabTarget::BulkEditTags => {
                    rsx! {
                        BulkEdit {
                            button_target: Modal::BulkEditTags,
                            button_text: "Edit Tags for Selected",
                            hidden_text: "Enable Bulk Edit Mode to edit tags for large groups of media.",
                        }
                    }
                }
                TabTarget::BulkAddToCollection => {
                    rsx! {
                        BulkEdit {
                            button_target: Modal::BulkAddToCollection,
                            button_text: "Add Selected to Collection",
                            hidden_text: "Enable Bulk Edit Mode to add media to collections in large groups.",
                        }
                    }
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct BulkEditProps {
    button_target: Modal,
    button_text: &'static str,
    hidden_text: &'static str,
}

#[component]
pub fn BulkEdit(props: BulkEditProps) -> Element {
    rsx! {
        div { class: "bulk-edit-options",
            div {
                class: "bulk-edit-controls",
                style: "margin-bottom: var(--space-4);",
                div { class: "form-group",
                    div { style: "display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-4);",
                        {
                            match BULK_EDIT() {
                                None => {
                                    rsx! {
                                        button {
                                            class: "btn btn-primary",
                                            onclick: move |_| {
                                                BULK_EDIT.with_mut(|v| *v = Some(HashSet::new()));
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
                                                BULK_EDIT.with_mut(|v| *v = None);
                                            },
                                            "Disable Bulk Edit Mode"
                                        }
                                    }
                                }
                            }
                        }

                        if let Some(media_uuids) = BULK_EDIT() {
                            span { style: "color: var(--text-secondary);",
                                "{media_uuids.len()} items selected"
                            }
                        }
                    }
                }
            }
        }

        if BULK_EDIT().is_some() {
            div {
                class: "bulk-actions",
                style: "display: flex; gap: var(--space-3); margin-top: var(--space-4);",
                button {
                    class: "btn btn-primary",
                    onclick: move |_| {
                        MODAL_STACK.with_mut(|v| { v.push(props.button_target.clone()) });
                    },
                    "{props.button_text}"
                }
                button {
                    class: "btn btn-secondary",
                    onclick: move |_| {
                        BULK_EDIT.with_mut(|v| *v = Some(HashSet::new()));
                    },
                    "Clear Selection"
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
                p { style: "margin: 0; color: var(--text-secondary);", "{props.hidden_text}" }
            }
        }
    }
}
