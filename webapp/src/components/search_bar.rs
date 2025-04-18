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
            style: "
                display: flex;
                align-items: center;
                gap: var(--space-2);
                margin-bottom: var(--space-6);
                background-color: var(--surface);
                padding: var(--space-3);
                border-radius: var(--radius-lg);
                box-shadow: var(--shadow-sm);
            ",
            form {
                class: "flex items-center",
                style: "flex: 1; display: flex; align-items: center;",
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
                    style: "flex: 1; margin-right: var(--space-2);",
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

            if !status.is_empty() {
                span {
                    class: "search-status",
                    style: "margin-left: var(--space-4); color: var(--text-tertiary);",
                    "{status}"
                }
            }

            if let Some(action_button) = props.action_button {
                div { class: "search-actions", {action_button} }
            }
        }
    }
}
