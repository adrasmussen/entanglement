use dioxus::prelude::*;
use tracing::error;

use crate::common::local_time;
use api::{comment::*, media::MediaUuid};

#[derive(Clone, PartialEq, Props)]
pub struct CommentListProps {
    comment_uuids: Memo<Vec<CommentUuid>>,
    media_uuid: Memo<MediaUuid>,
    update_signal: Signal<()>,
}

#[component]
pub fn CommentList(props: CommentListProps) -> Element {
    rsx! {
        ErrorBoundary {
            handle_error: |error: ErrorContext| {
                rsx! {
                    if let Some(error_ui) = error.show() {
                        {error_ui}
                    } else {
                        div { "CommentList encountered an error.  Check the logs or reach out the the administrators." }
                    }
                }
            },
            CommentListInner {
                comment_uuids: props.comment_uuids,
                media_uuid: props.media_uuid,
                update_signal: props.update_signal,
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
struct CommentListInnerProps {
    comment_uuids: Memo<Vec<CommentUuid>>,
    media_uuid: Memo<MediaUuid>,
    update_signal: Signal<()>,
}

#[component]
fn CommentListInner(props: CommentListInnerProps) -> Element {
    let comment_uuids = props.comment_uuids;
    let media_uuid = *props.media_uuid.read();
    let mut update_signal = props.update_signal;

    let mut new_comment = use_signal(String::new);
    let mut status_signal = use_signal(String::new);

    let comments_future = use_resource(move || {
        async move {
            let mut comments = Vec::new();

            for comment_uuid in comment_uuids() {
                match get_comment(&GetCommentReq { comment_uuid }).await {
                    Ok(resp) => comments.push((comment_uuid, resp.comment)),
                    Err(err) => {
                        error!("Failed to fetch comment for {comment_uuid}: {err}")
                    }
                }
            }

            // Sort comments by timestamp (newest first)
            comments.sort_by(|a, b| b.1.mtime.cmp(&a.1.mtime));
            comments
        }
    });

    let comments = &*comments_future.read();
    let comments = comments.clone();

    rsx! {
        div { class: "comments-container",
            form {
                class: "comment-form",
                style: "margin-bottom: var(--space-4); background-color: var(--neutral-50); padding: var(--space-3); border-radius: var(--radius-md);",
                onsubmit: move |evt| async move {
                    evt.prevent_default();
                    let comment_text = new_comment();
                    if comment_text.trim().is_empty() {
                        status_signal.set("Comment cannot be empty".into());
                        return;
                    }
                    match add_comment(
                            &AddCommentReq {
                                comment: Comment {
                                    media_uuid,
                                    mtime: 0,
                                    uid: String::from(""),
                                    text: comment_text,
                                },
                            },
                        )
                        .await
                    {
                        Ok(_) => {
                            new_comment.set(String::new());
                            status_signal.set("Comment added successfully".into());
                            update_signal.set(());
                        }
                        Err(err) => {
                            status_signal.set(format!("Error: {}", err));
                        }
                    }
                },

                div {
                    class: "form-group",
                    style: "margin-bottom: var(--space-2);",
                    label { class: "form-label", "Add a comment" }
                    textarea {
                        class: "form-textarea",
                        placeholder: "Write your comment here...",
                        rows: 2,
                        value: "{new_comment}",
                        oninput: move |evt| new_comment.set(evt.value().clone()),
                    }
                }

                div {
                    class: "form-actions",
                    style: "display: flex; justify-content: space-between; align-items: center;",
                    if !status_signal().is_empty() {
                        span { class: "status-message", "{status_signal()}" }
                    }

                    button { class: "btn btn-primary btn-sm", r#type: "submit", "Post Comment" }
                }
            }

            match comments {
                Some(comments) => {
                    rsx! {
                        div {
                            class: "comments-list",
                            style: "display: flex; flex-direction: column; gap: var(--space-3);",
                            if comments.is_empty() {
                                div {
                                    class: "empty-state",
                                    style: "padding: var(--space-3); text-align: center; color: var(--text-tertiary);",
                                    "No comments yet. Be the first to add one!"
                                }
                            } else {
                                for (comment_uuid , comment) in comments {
                                    div {
                                        class: "comment-item",
                                        style: "padding: var(--space-3); border-radius: var(--radius-md); border: 1px solid var(--border); background-color: var(--surface);",
                                        div {
                                            class: "comment-header",
                                            style: "display: flex; justify-content: space-between; margin-bottom: var(--space-2);",
                                            div { class: "comment-author", style: "font-weight: 500;", "{comment.uid}" }
                                            div {
                                                class: "comment-time",
                                                style: "font-size: 0.875rem; color: var(--text-tertiary);",
                                                "{local_time(comment.mtime)}"
                                            }
                                        }
                                        div { class: "comment-text", style: "white-space: pre-wrap;", "{comment.text}" }
                                        div {
                                            class: "comment-actions",
                                            style: "display: flex; justify-content: flex-end; margin-top: var(--space-2);",
                                            button {
                                                class: "btn btn-sm btn-danger",
                                                onclick: move |_| {
                                                    crate::components::modal::MODAL_STACK
                                                        .with_mut(|v| {
                                                            v.push(
                                                                crate::components::modal::Modal::DeleteComment(
                                                                    comment_uuid,
                                                                    media_uuid,
                                                                ),
                                                            )
                                                        });
                                                },
                                                "Delete"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                None => {
                    rsx! {
                        div {
                            class: "comments-list",
                            style: "display: flex; flex-direction: column; gap: var(--space-3);",
                            for _ in 0..2 {
                                div {
                                    class: "comment-skeleton",
                                    style: "padding: var(--space-3); border-radius: var(--radius-md); border: 1px solid var(--border); background-color: var(--surface);",
                                    div { style: "display: flex; justify-content: space-between; margin-bottom: var(--space-2);",
                                        div { class: "skeleton", style: "width: 100px; height: 18px;" }
                                        div { class: "skeleton", style: "width: 80px; height: 16px;" }
                                    }
                                    div {
                                        class: "skeleton",
                                        style: "height: 18px; width: 90%; margin-bottom: var(--space-1);",
                                    }
                                    div {
                                        class: "skeleton",
                                        style: "height: 18px; width: 75%; margin-bottom: var(--space-1);",
                                    }
                                    div { class: "skeleton", style: "height: 18px; width: 60%;" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
