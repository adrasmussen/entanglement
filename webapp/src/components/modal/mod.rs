// webapp/src/components/modal/mod.rs

use dioxus::prelude::*;

use api::{album::AlbumUuid, comment::CommentUuid, media::MediaUuid};

mod comments;
use comments::DeleteCommentModal;

mod albums;
use albums::{CreateAlbumModal, RmFromAlbumModal};

mod enhanced_media_modal;
use enhanced_media_modal::EnhancedMediaModal;

mod media_detail_modal;
use media_detail_modal::MediaDetailModal;

// global modal signal
//
// rather than having each page have its own modal signal logic, we define a global
// signal so that moving between pages via modal works
pub static MODAL_STACK: GlobalSignal<Vec<Modal>> = Signal::global(|| Vec::new());

// Modal
//
// this enumerates all of the modal boxes we can display, and what the relevant
// data is to show the correct box.  pushing this onto the modal stack will
// trigger the ModalBox, below
pub enum Modal {
    ShowMedia(MediaUuid),
    EnhancedImageView(MediaUuid),
    RmMediaFromAlbumConfirmation(MediaUuid, AlbumUuid),
    DeleteCommentConfirmation(CommentUuid, MediaUuid),
    ShowAlbum(AlbumUuid),
    CreateAlbum, // New variant for album creation
}

// ModalBox
//
// this is the struct that, once included into another element, actually displays
// the modal on the top of the stack (from the global signal).  the meaning of
// the update_signal is dependent on the calling component, and is intended to be
// a more targeted way to know when to re-run use_resource() hooks.
#[derive(Clone, PartialEq, Props)]
pub struct ModalBoxProps {
    update_signal: Signal<()>,
}

#[component]
pub fn ModalBox(props: ModalBoxProps) -> Element {
    let update_signal = props.update_signal;

    match MODAL_STACK.read().last() {
        Some(val) => {
            match *val {
                Modal::ShowMedia(media_uuid) => rsx! {
                    MediaDetailModal { update_signal, media_uuid }
                },
                Modal::EnhancedImageView(media_uuid) => rsx! {
                    EnhancedMediaModal { media_uuid }
                },
                Modal::ShowAlbum(album_uuid) => rsx! {
                    ModernModal {
                        title: "Album Details",

                        // Album details content here...

                        footer: rsx! {
                            button {
                                class: "btn btn-secondary",
                                onclick: move |_| {
                                    MODAL_STACK.with_mut(|v| v.pop());
                                },
                                "Close"
                            }
                        },
                    }
                },
                Modal::DeleteCommentConfirmation(comment_uuid, media_uuid) => {
                    rsx! {
                        DeleteCommentModal {
                            update_signal,
                            comment_uuid,
                            media_uuid,
                        }
                    }
                }
                Modal::RmMediaFromAlbumConfirmation(media_uuid, album_uuid) => {
                    rsx! {
                        RmFromAlbumModal {
                            update_signal,
                            media_uuid,
                            album_uuid,
                        }
                    }
                }
                Modal::CreateAlbum => {
                    rsx! {
                        CreateAlbumModal { update_signal }
                    }
                }
                _ => rsx! {
                    span { "not implemented" }
                },
            }
        }
        None => rsx! {},
    }
}

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
