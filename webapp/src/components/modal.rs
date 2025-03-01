use dioxus::prelude::*;

use crate::common::modal::MODAL_STACK;

#[derive(Clone, PartialEq, Props)]
pub struct ModalProps {
    title: String,
    #[props(default)]
    size: ModalSize,
    #[props(default)]
    disable_close: bool,
    children: Element,
    #[props(default)]
    footer: Option<Element>,
}

#[derive(Clone, PartialEq)]
pub enum ModalSize {
    Small,
    Medium,
    Large,
    Full,
}

impl Default for ModalSize {
    fn default() -> Self {
        ModalSize::Medium
    }
}

#[component]
pub fn ModernModal(props: ModalProps) -> Element {
    // Determine width based on size
    let width = match props.size {
        ModalSize::Small => "max-width: 400px;",
        ModalSize::Medium => "max-width: 600px;",
        ModalSize::Large => "max-width: 800px;",
        ModalSize::Full => "max-width: 95%;",
    };

    rsx! {
        div {
            class: "modal-overlay",
            // Clicking overlay closes modal unless disabled
            onclick: move |evt| {
                evt.stop_propagation();
                if !props.disable_close {
                    MODAL_STACK
                        .with_mut(|v| {
                            if !v.is_empty() {
                                v.pop();
                            }
                        });
                }
            },
            div {
                class: "modal-content",
                style: "{width}",
                // Stop click propagation to prevent closing when clicking content
                onclick: move |evt| evt.stop_propagation(),

                div { class: "modal-header",
                    h2 { class: "modal-title", "{props.title}" }
                    if !props.disable_close {
                        button {
                            class: "btn-close",
                            onclick: move |_| {
                                MODAL_STACK
                                    .with_mut(|v| {
                                        if !v.is_empty() {
                                            v.pop();
                                        }
                                    });
                            },
                            "×"
                        }
                    }
                }

                div { class: "modal-body", {props.children} }

                if let Some(footer) = &props.footer {
                    div { class: "modal-footer", {footer.clone()} }
                }
            }
        }
    }
}

// Helper function to create a standardized footer with common actions
pub fn _modal_footer_buttons(
    primary_text: &str,
    primary_action: EventHandler<MouseEvent>,
    secondary_text: &str,
    secondary_action: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        div { class: "modal-buttons",
            button {
                class: "btn btn-secondary",
                onclick: move |evt| secondary_action.call(evt),
                "{secondary_text}"
            }
            button {
                class: "btn btn-primary",
                onclick: move |evt| primary_action.call(evt),
                "{primary_text}"
            }
        }
    }
}
