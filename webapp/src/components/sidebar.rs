use dioxus::prelude::*;

pub static SIDEBAR_SIGNAL: GlobalSignal<Option<Sidebar>> = Signal::global(|| None);

#[derive(Clone, Debug, PartialEq)]
pub enum Sidebar {
    Advanced,
}

#[component]
pub fn SidebarBox() -> Element {
    match &*SIDEBAR_SIGNAL.read() {
        Some(val) => match *val {
            Sidebar::Advanced => rsx! {
                AdvancedSidebar {}
            },
        },
        None => rsx! {},
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct SidebarInnerProps {
    title: String,
    #[props(default)]
    side: SidebarSide,
    children: Element,
}

#[derive(Clone, Default, PartialEq)]
pub enum SidebarSide {
    #[default]
    Right,
    Left,
}

#[component]
pub fn SidebarInner(props: SidebarInnerProps) -> Element {
    rsx! {
        // Clicking overlay closes sidebar unless disabled
        // if !sidebar_state.is_none() {
        //     div {
        //         class: "sidebar-overlay",
        //         onclick: move |_| {
        //             SIDEBAR_SIGNAL.with_mut(|state| {
        //                 state = None;
        //             });
        //         },
        //     }
        // }
        div { class: "sidebar sidebar-open",
            div { class: "sidebar-content",

                div { class: "sidebar-header",
                    h2 { class: "sidebar-title", "{props.title}" }
                    button {
                        class: "sidebar-close",
                        onclick: move |_| {
                            SIDEBAR_SIGNAL
                                .with_mut(|state| {
                                    *state = None;
                                });
                        },
                        "×"
                    }
                }

                div { class: "sidebar-body", {props.children} }
            }
        }
    }
}

#[component]
fn AdvancedSidebar() -> Element {
    rsx! {
        SidebarInner { title: "advanced options", side: SidebarSide::Right,
            div {
                p { "advanced sidebar" }
            }
        }
    }
}
