use dioxus::prelude::*;
use gloo_timers::callback::Timeout;

use crate::components::modal::{MODAL_STACK, ModalSize, ModernModal};
use api::{
    comment::{CommentUuid, DeleteCommentReq, delete_comment},
    media::MediaUuid,
};

// Confirmation modal for deleting comments
#[derive(Clone, PartialEq, Props)]
pub struct DeleteCommentModalProps {
    update_signal: Signal<()>,
    comment_uuid: CommentUuid,
    media_uuid: MediaUuid,
}

#[component]
pub fn DeleteCommentModal(props: DeleteCommentModalProps) -> Element {
    let comment_uuid = props.comment_uuid;
    let media_uuid = props.media_uuid;
    let mut update_signal = props.update_signal;
    let mut status_message = use_signal(|| String::new());

    let footer = rsx! {
        span { class: "status-message", "{status_message}" }
        div {
            class: "modal-buttons",
            style: "display: flex; gap: var(--space-4); justify-content: flex-end;",
            button {
                class: "btn btn-secondary",
                onclick: move |_| {
                    MODAL_STACK.with_mut(|v| v.pop());
                },
                "Cancel"
            }
            button {
                class: "btn btn-danger",
                onclick: move |_| async move {
                    match delete_comment(
                            &DeleteCommentReq {
                                comment_uuid: comment_uuid,
                            },
                        )
                        .await
                    {
                        Ok(_) => {
                            status_message.set("Comment deleted".into());
                            update_signal.set(());
                            let task = Timeout::new(
                                1500,
                                move || {
                                    MODAL_STACK.with_mut(|v| v.pop());
                                },
                            );
                            task.forget();
                        }
                        Err(err) => {
                            status_message.set(format!("Error: {}", err));
                        }
                    }
                },
                "Delete Comment"
            }
        }
    };

    rsx! {
        ModernModal { title: "Confirm Deletion", size: ModalSize::Small, footer,

            div { class: "confirmation-content",
                p { class: "confirmation-message",
                    "Are you sure you want to delete this comment? This action cannot be undone."
                }

                div {
                    class: "media-info",
                    style: "margin-top: var(--space-4); padding: var(--space-3); background-color: var(--neutral-50); border-radius: var(--radius-md);",
                    p { "Media ID: {media_uuid}" }
                    p { "Comment ID: {comment_uuid}" }
                }
            }
        }
    }
}
