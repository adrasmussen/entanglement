use dioxus::prelude::*;

mod advanced;
pub use advanced::AdvancedSidebar;

#[derive(Clone, PartialEq, Props)]
pub struct SidebarInnerProps {
    show_signal: Signal<bool>,
    title: String,
    children: Element,
}

#[component]
pub fn SidebarInner(props: SidebarInnerProps) -> Element {
    let mut show_signal = props.show_signal;

    // TODO -- use the right css here
    if !show_signal() {
        return rsx! {};
    }

    rsx! {
        div { class: "sidebar sidebar-open",
            div { class: "sidebar-content",

                div { class: "sidebar-header",
                    h2 { class: "sidebar-title", "{props.title}" }
                    button {
                        class: "sidebar-close",
                        onclick: move |_| { show_signal.set(false) },
                        "×"
                    }
                }

                div { class: "sidebar-body", {props.children} }
            }
        }
    }
}
