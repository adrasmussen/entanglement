use dioxus::prelude::*;

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
        div { class: "search-bar",
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
