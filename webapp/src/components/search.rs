use dioxus::prelude::*;

use crate::common::storage::set_local_storage;

#[derive(Clone, PartialEq, Props)]
pub struct SearchBarProps {
    search_signal: Signal<String>,
    storage_key: &'static str,
    placeholder: &'static str,
    #[props(default)]
    status: String,
    #[props(default)]
    action_button: Option<Element>,
}

#[component]
pub fn SearchBar(props: SearchBarProps) -> Element {
    let mut search_signal = props.search_signal;
    let storage_key = props.storage_key;
    let placeholder = props.placeholder;
    let status = props.status.clone();

    rsx! {
        div {
            class: "search-bar",
            form {
                style: "flex: 1; display: flex; align-items: center; gap: var(--space-2);",
                onsubmit: move |event| async move {
                    let filter = match event.values().get("search_filter") {
                        Some(val) => val.as_value(),
                        None => String::from(""),
                    };
                    search_signal.set(filter.clone());
                    set_local_storage(storage_key, filter);
                },
                div {
                    class: "search-input",
                    input {
                        class: "form-input",
                        style: "width: 100%;",
                        name: "search_filter",
                        r#type: "text",
                        placeholder: "{placeholder}",
                        value: "{search_signal()}",
                    }
                }
                button { class: "btn btn-primary", r#type: "submit", "Search" }
            }

            if let Some(action_button) = props.action_button {
                div { class: "search-actions", {action_button} }
            }

            if !status.is_empty() {
                span {
                    class: "search-status",
                    style: "margin-left: var(--space-4); color: var(--text-tertiary);",
                    "{status}"
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct CompactSearchBarProps {
    search_signal: Signal<String>,
    placeholder: &'static str,
}

#[component]
pub fn CompactSearchBar(props: CompactSearchBarProps) -> Element {
    let mut search_signal = props.search_signal;
    let placeholder = props.placeholder;

    rsx! {
        div {
            class: "search-bar",
            form {
                class: "form-group",
                style: "width: 100%;",
                onsubmit: move |event| async move {
                    let filter = match event.values().get("search_filter") {
                        Some(val) => val.as_value(),
                        None => String::from(""),
                    };
                    search_signal.set(filter.clone());
                },
                div { style: "display: flex; gap: var(--space-2); align-items: center;",
                    input {
                        class: "form-input",
                        r#type: "text",
                        name: "search_filter",
                        value: "{search_signal()}",
                        placeholder: "{placeholder}",
                        style: "flex: 1;",
                    }
                    button { class: "btn btn-primary", r#type: "submit", "Search" }
                }
            }
        }
    }
}
