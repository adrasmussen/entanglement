use dioxus::prelude::*;

#[derive(Clone, PartialEq, Props)]
pub struct ModalSearchBarProps {
    search_signal: Signal<String>,
    placeholder: &'static str,
}

#[component]
pub fn ModalSearchBar(props: ModalSearchBarProps) -> Element {
    let mut search_signal = props.search_signal;
    let placeholder = props.placeholder;

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
