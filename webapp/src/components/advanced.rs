use std::collections::HashSet;

use dioxus::prelude::*;

use crate::components::modal::{Modal, MODAL_STACK};
use api::media::MediaUuid;

#[derive(Clone, PartialEq)]
enum TabTarget {
    Search,
    _BulkTagEdit,
    BulkAddToCollection,
}

#[derive(Clone, PartialEq, Props)]
struct AdvancedContentProps {
    tab_signal: Signal<TabTarget>,
    media_search_signal: Signal<String>,
    bulk_edit_mode_signal: Signal<bool>,
    selected_media_signal: Signal<HashSet<MediaUuid>>,
}

#[component]
fn AdvancedContent(props: AdvancedContentProps) -> Element {
    let tab_signal = props.tab_signal;
    let mut media_search_signal = props.media_search_signal;
    let mut bulk_edit_mode_signal = props.bulk_edit_mode_signal;
    let mut selected_media_signal = props.selected_media_signal;

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
                TabTarget::_BulkTagEdit => {
                    rsx! { "to do" }
                }
                TabTarget::BulkAddToCollection => {
                    rsx! {
                        div { class: "bulk-collection-options",
                            div { class: "bulk-edit-controls", style: "margin-bottom: var(--space-4);",
                                div { class: "form-group",
                                    div { style: "display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-4);",
                                        button {
                                            class: if bulk_edit_mode_signal() { "btn btn-danger" } else { "btn btn-primary" },
                                            onclick: move |_| {
                                                if bulk_edit_mode_signal() {
                                                    selected_media_signal.set(HashSet::new());
                                                }

                                                bulk_edit_mode_signal.set(!bulk_edit_mode_signal());
                                            },

                                            if bulk_edit_mode_signal() {
                                                "Disable Bulk Edit Mode"
                                            } else {
                                                "Enable Bulk Edit Mode"
                                            }
                                        }

                                        if bulk_edit_mode_signal() {
                                            span { style: "color: var(--text-secondary);",
                                                "{selected_media_signal().len()} items selected"
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if bulk_edit_mode_signal() {
                            div {
                                class: "bulk-actions",
                                style: "display: flex; gap: var(--space-3); margin-top: var(--space-4);",
                                button {
                                    class: "btn btn-primary",
                                    disabled: selected_media_signal().is_empty(),
                                    onclick: move |_| {
                                        if !selected_media_signal().is_empty() {
                                            MODAL_STACK
                                                .with_mut(|v| {
                                                    v.push(
                                                        Modal::BulkAddToCollection(
                                                            selected_media_signal().clone(),
                                                        ),
                                                    );
                                                });
                                        }
                                    },
                                    "Add Selected to Collection"
                                }
                                button {
                                    class: "btn btn-secondary",
                                    disabled: selected_media_signal().is_empty(),
                                    onclick: move |_| {
                                        selected_media_signal.set(HashSet::new());
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
                                p { style: "margin: 0; color: var(--text-secondary);",
                                    "Enable Bulk Edit Mode to add media to collections in large groups."
                                }
                            }
                        }
                    }
                }
            }
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
            class: if &*tab_signal.read() == &target { "tab-button active" } else { "tab-button" },
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
                + if &*tab_signal.read() == &target {
                    "color: var(--primary); border-bottom-color: var(--primary);"
                } else {
                    ""
                },
            onclick: move |_| tab_signal.set(target.clone()),

            "{text}"
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct AdvancedContainerProps {
    media_search_signal: Signal<String>,
    bulk_edit_mode_signal: Signal<bool>,
    selected_media_signal: Signal<HashSet<MediaUuid>>,
}

#[component]
pub fn AdvancedContainer(props: AdvancedContainerProps) -> Element {
    let media_search_signal = props.media_search_signal;
    let bulk_edit_mode_signal = props.bulk_edit_mode_signal;
    let selected_media_signal = props.selected_media_signal;

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
                style: "
                    display: flex;
                    border-bottom: 1px solid var(--neutral-200);
                    margin-bottom: var(--space-4);
                ",
                AdvancedTab {
                    tab_signal,
                    target: TabTarget::Search,
                    text: "Search Options",
                }
                AdvancedTab {
                    tab_signal,
                    target: TabTarget::_BulkTagEdit,
                    text: "Bulk Edit Tags",
                }
                AdvancedTab {
                    tab_signal,
                    target: TabTarget::BulkAddToCollection,
                    text: "Bulk Add to Collection",
                }
            }

            AdvancedContent {
                tab_signal,
                media_search_signal,
                bulk_edit_mode_signal,
                selected_media_signal,
            }
        }
    }
}
