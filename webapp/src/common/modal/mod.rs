use dioxus::prelude::*;

use crate::common::style;
use api::{album::AlbumUuid, comment::CommentUuid, library::LibraryUuid, media::MediaUuid};

mod media;
use media::ShowMediaBox;

mod album;
use album::{CreateAlbumBox, DeleteAlbumBox, ShowAlbumBox};

mod comment;
use comment::{AddCommentBox, DeleteCommentBox};

pub static MODAL_STACK: GlobalSignal<Vec<Modal>> = Signal::global(|| Vec::new());

// Modal
//
// this enumerates all of the modal boxes we can display, and what the relevant
// data is to show the correct box.  pushing this onto the modal stack will
// trigger the ModalBox, below
pub enum Modal {
    ShowMedia(MediaUuid),
    ShowAlbum(AlbumUuid),
    CreateAlbum,
    DeleteAlbum(AlbumUuid),
    UpdateAlbum(AlbumUuid),
    AddMediaToAlbum(MediaUuid),
    RmMediaFromAlbum(MediaUuid, AlbumUuid),
    ShowLibrary(LibraryUuid),
    AddLibrary,
    AddComment(MediaUuid),
    DeleteComment(CommentUuid),
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
    rsx! {
        div {
            style { "{style::MODAL}" }
            div { class: "modal",
                div { class: "modal-content",
                    div { class: "modal-header",
                        span {
                            class: "close",
                            onclick: move |_| {
                                MODAL_STACK.with_mut(|v| v.pop());
                            },
                            "X"
                        }
                    }
                    match MODAL_STACK.read().last() {
                        Some(val) => {
                            match *val {
                                Modal::ShowMedia(media_uuid) => rsx! {
                                    ShowMediaBox { media_uuid }
                                },
                                Modal::ShowAlbum(album_uuid) => rsx! {
                                    ShowAlbumBox { album_uuid }
                                },
                                Modal::CreateAlbum => rsx! {
                                    CreateAlbumBox {}
                                },
                                Modal::DeleteAlbum(album_uuid) => rsx! {
                                    DeleteAlbumBox { update_signal, album_uuid }
                                },
                                Modal::UpdateAlbum(album_uuid) => rsx! {
                                    ModalErr { err: "not implemented" }
                                },
                                Modal::AddMediaToAlbum(media_uuid) => rsx! {
                                    ModalErr { err: "not implemented" }
                                },
                                Modal::RmMediaFromAlbum(media_uuid, album_uuid) => {
                                    rsx! {
                                        ModalErr { err: "not implemented" }
                                    }
                                }
                                Modal::ShowLibrary(library_uuid) => rsx! {
                                    ModalErr { err: "not implemented" }
                                },
                                Modal::AddLibrary => rsx! {
                                    ModalErr { err: "not implemented" }
                                },
                                Modal::AddComment(media_uuid) => rsx! {
                                    AddCommentBox { update_signal, media_uuid }
                                },
                                Modal::DeleteComment(comment_uuid) => rsx! {
                                    DeleteCommentBox { update_signal, comment_uuid }
                                },
                            }
                        }
                        None => return rsx! {},
                    }
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
struct ModalErrProps {
    err: String,
}

#[component]
fn ModalErr(props: ModalErrProps) -> Element {
    rsx! {
        div { class: "modal-body",
            span { "{props.err}" }
        }
    }
}

pub fn modal_err(err: impl Into<String>) -> Element {
    rsx! {
        ModalErr { err: err.into() }
    }
}
