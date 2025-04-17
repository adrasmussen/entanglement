use std::collections::HashSet;

use dioxus::prelude::*;

use api::{
    collection::CollectionUuid, comment::CommentUuid, library::LibraryUuid, media::MediaUuid,
};

mod comments;
use comments::DeleteCommentModal;

mod collections;
use collections::{
    AddMediaToCollectionModal, CreateCollectionModal, DeleteCollectionModal, EditCollectionModal,
    RmFromCollectionModal, BulkAddToCollectionModal,
};

mod enhanced_media_modal;
use enhanced_media_modal::EnhancedMediaModal;

mod library;
use library::{StartTaskModal, StopTaskModal};

mod search;

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
    EnhancedImageView(MediaUuid),
    DeleteComment(CommentUuid, MediaUuid),
    CreateCollection,
    EditCollection(CollectionUuid),
    DeleteCollection(CollectionUuid),
    AddMediaToCollection(MediaUuid),
    RmMediaFromCollection(MediaUuid, CollectionUuid),
    BulkAddToCollection(HashSet<MediaUuid>),
    StartTask(LibraryUuid),
    StopTask(LibraryUuid),
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
        Some(val) => match *val {
            Modal::EnhancedImageView(media_uuid) => rsx! {
                EnhancedMediaModal { media_uuid }
            },
            Modal::DeleteComment(comment_uuid, media_uuid) => {
                rsx! {
                    DeleteCommentModal { update_signal, comment_uuid, media_uuid }
                }
            }
            Modal::CreateCollection => {
                rsx! {
                    CreateCollectionModal { update_signal }
                }
            }
            Modal::EditCollection(collection_uuid) => {
                rsx! {
                    EditCollectionModal { update_signal, collection_uuid }
                }
            }
            Modal::DeleteCollection(collection_uuid) => {
                rsx! {
                    DeleteCollectionModal { update_signal, collection_uuid }
                }
            }
            Modal::AddMediaToCollection(media_uuid) => {
                rsx! {
                    AddMediaToCollectionModal { update_signal, media_uuid }
                }
            }
            Modal::RmMediaFromCollection(media_uuid, collection_uuid) => {
                rsx! {
                    RmFromCollectionModal {
                        update_signal,
                        media_uuid,
                        collection_uuid,
                    }
                }
            }
            Modal::BulkAddToCollection(ref media_uuids) => {
                rsx! {
                    BulkAddToCollectionModal { update_signal, media_uuids: media_uuids.clone() }
                }
            }
            Modal::StartTask(library_uuid) => {
                rsx! {
                    StartTaskModal { update_signal, library_uuid }
                }
            }
            Modal::StopTask(library_uuid) => {
                rsx! {
                    StopTaskModal { update_signal, library_uuid }
                }
            }
        },
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
                    div {
                        class: "modal-footer",
                        style: "display: flex; align-items: center; justify-content: space-between; gap: var(--space-4);",
                        {footer.clone()}
                    }
                }
            }
        }
    }
}
