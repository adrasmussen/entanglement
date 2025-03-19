use dioxus::prelude::*;

#[derive(Clone, PartialEq)]
enum TabTarget {
    Text,
    Date,
    Metadata,
    Similar,
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
                ".to_string() + if &*tab_signal.read() == &target{ "color: var(--primary); border-bottom-color: var(--primary);" } else { "" },
            onclick: move |_| tab_signal.set(target.clone()),

            "{text}"
        }
    }
}
