use dioxus::prelude::*;

#[derive(Clone, PartialEq)]
enum TabTarget {
    _Search,
    _BulkTagEdit,
    _BulkCollectionEdit,
}

#[derive(Clone, PartialEq, Props)]
struct AdvancedContentProps {
    tab_signal: Signal<TabTarget>,
    media_search_signal: Signal<String>,
}

#[component]
fn AdvancedContent(props: AdvancedContentProps) -> Element {
    let tab_signal = props.tab_signal;

    rsx! {
        div { class: "tab-content", style: "min-height: 200px;",

            match tab_signal() {
                TabTarget::_Search => {
                    rsx! { "to do" }
                }
                TabTarget::_BulkTagEdit => {
                    rsx! { "to do" }
                }
                TabTarget::_BulkCollectionEdit => {
                    rsx! { "to do" }
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
struct AdvancedContainerProps {
    media_search_signal: Signal<String>,
}

#[component]
fn AdvancedContainer(props: AdvancedContainerProps) -> Element {
    let media_search_signal = props.media_search_signal;

    let tab_signal: Signal<TabTarget> = use_signal(|| TabTarget::_Search);

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
        }
        div {
            class: "tabs-navigation",
            style: "
                display: flex;
                border-bottom: 1px solid var(--neutral-200);
                margin-bottom: var(--space-4);
            ",
            AdvancedTab {
                tab_signal,
                target: TabTarget::_Search,
                text: "Search Options",
            }
            AdvancedTab {
                tab_signal,
                target: TabTarget::_BulkTagEdit,
                text: "Show Collections",
            }
            AdvancedTab {
                tab_signal,
                target: TabTarget::_BulkCollectionEdit,
                text: "Bulk Select",
            }
        }

        AdvancedContent { tab_signal, media_search_signal }
    }
}
