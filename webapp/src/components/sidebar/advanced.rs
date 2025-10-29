use std::collections::HashMap;

use dioxus::prelude::*;

use crate::components::sidebar::SidebarInner;
use api::WebError;

#[derive(Clone, PartialEq, Props)]
pub struct AdvancedSidebarProps {
    show_signal: Signal<bool>,
    tabs: HashMap<String, Element>,
}

#[component]
pub fn AdvancedSidebar(props: AdvancedSidebarProps) -> Element {
    let show_signal = props.show_signal;
    let tabs = props.tabs;

    let mut tab_names = tabs.keys().cloned().collect::<Vec<String>>();

    tab_names.sort();

    let default_tab = match tab_names.first() {
        Some(v) => v.clone(),
        None => return rsx! {},
    };

    let mut tab_signal = use_signal(|| default_tab);

    rsx! {
        SidebarInner {
            show_signal,
            title: "Advanced Options",
            children: rsx! {
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
            },
        }
    }
}
