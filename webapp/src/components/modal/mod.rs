use std::collections::HashSet;

use dioxus::prelude::*;

use api::{
    collection::CollectionUuid, comment::CommentUuid, library::LibraryUuid, media::MediaUuid,
};

mod comments;
use comments::DeleteCommentModal;

mod collections;
use collections::{
    AddMediaToCollectionModal, BulkAddToCollectionModal, CreateCollectionModal,
    DeleteCollectionModal, EditCollectionModal, RmFromCollectionModal,
};

mod library;
use library::{StartTaskModal, StopTaskModal};

mod media;
use media::{BulkEditTagsModal, EnhancedMediaModal};

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
#[derive(Clone, Debug, PartialEq)]
pub enum Modal {
    EnhancedImageView(MediaUuid),
    DeleteComment(CommentUuid, MediaUuid),
    CreateCollection,
    EditCollection(CollectionUuid),
    DeleteCollection(CollectionUuid),
    AddMediaToCollection(MediaUuid),
    RmMediaFromCollection(MediaUuid, CollectionUuid),
    BulkAddToCollection(HashSet<MediaUuid>),
    BulkEditTagsModal(HashSet<MediaUuid>),
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
            Modal::BulkEditTagsModal(ref media_uuids) => {
                rsx! {
                    BulkEditTagsModal { update_signal, media_uuids: media_uuids.clone() }
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

#[derive(Clone, PartialEq, Props)]
pub struct ProgressBarProps {
    processing_count: Signal<i64>,
    success_count: Signal<i64>,
    error_count: Signal<i64>,
    media_count: i64,
}

#[component]
pub fn ProgressBar(props: ProgressBarProps) -> Element {
    let processing_count = props.processing_count;
    let success_count = props.success_count;
    let error_count = props.error_count;
    let media_count = props.media_count;
    rsx! {
        if processing_count() > 0 && processing_count() < media_count {
            div {
                class: "progress-container",
                style: "margin-bottom: var(--space-4); padding: var(--space-3); background-color: var(--neutral-50); border-radius: var(--radius-md);",
                p { "Processing... {processing_count()} of {media_count} items" }
                div {
                    class: "progress-bar",
                    style: "height: 8px; background-color: var(--neutral-200); border-radius: var(--radius-full); overflow: hidden; margin-top: var(--space-2);",
                    div {
                        style: format!(
                            "height: 100%; background-color: var(--primary); width: {}%;",
                            (processing_count() as f64 / media_count as f64 * 100.0) as i64,
                        ),
                    }
                }
                div { style: "display: flex; justify-content: space-between; margin-top: var(--space-2); font-size: 0.875rem; color: var(--text-tertiary);",
                    span { "Success: {success_count()}" }
                    if error_count() > 0 {
                        span { style: "color: var(--error);", "Failed: {error_count()}" }
                    }
                }
            }
        } else if success_count() > 0 || error_count() > 0 {
            div {
                class: "result-container",
                style: "margin-bottom: var(--space-4); padding: var(--space-3); border-radius: var(--radius-md);",
                style: if error_count() > 0 { "background-color: rgba(239, 68, 68, 0.1);" } else { "background-color: rgba(16, 185, 129, 0.1);" },
                if error_count() == 0 {
                    p { style: "color: var(--success); font-weight: 500;",
                        "Successfully modified {success_count()} items"
                    }
                } else {
                    p { "Modified {success_count()} items, {error_count()} failed" }
                }
            }
        }
    }
}
