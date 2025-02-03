use dioxus::prelude::*;

use crate::common::{local_time, modal::MODAL_STACK};
use api::{comment::*, media::MediaUuid};

#[derive(Clone, PartialEq, Props)]
pub struct AddCommentBoxProps {
    media_uuid: MediaUuid,
}

#[component]
pub fn AddCommentBox(props: AddCommentBoxProps) -> Element {
    let media_uuid = props.media_uuid;

    let status_signal = use_signal(|| String::from(""));

    rsx! {
        div { class: "modal-body", grid_template_columns: "1fr",
            div {
                h3 { "Add comment" }
                form {
                    onsubmit: move |event| async move {
                        let mut status_signal = status_signal;
                        let text = match event.values().get("text") {
                            Some(val) => val.as_value(),
                            None => String::from(""),
                        };
                        if text.as_str() == "" {
                            status_signal.set(String::from("Error adding comment: empty text"));
                            return;
                        }
                        let result = match add_comment(
                                &AddCommentReq {
                                    comment: Comment {
                                        media_uuid: media_uuid,
                                        mtime: 0,
                                        uid: String::from(""),
                                        text: text,
                                    },
                                },
                            )
                            .await
                        {
                            Ok(_) => {
                                MODAL_STACK.with_mut(|v| v.pop());
                                return;
                            }
                            Err(err) => format!("Error creating album: {}", err.to_string()),
                        };
                        status_signal.set(result)
                    },

                    label { "Text" }
                    textarea {
                        name: "text",
                        rows: "8",
                        width: "100%",
                        value: "",
                    }
                    div { grid_template_columns: "1fr 1fr",
                        input { r#type: "submit", value: "Create comment" }
                        button {
                            onclick: move |_| {
                                MODAL_STACK.with_mut(|v| v.pop());
                            },
                            "Cancel"
                        }
                    }
                }
            }
        }
        div { class: "modal-footer",
            span { "{status_signal()}" }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct DeleteCommentBoxProps {
    comment_uuid: CommentUuid,
}

#[component]
pub fn DeleteCommentBox(props: DeleteCommentBoxProps) -> Element {
    let comment_uuid = props.comment_uuid;

    let status_signal = use_signal(|| String::from(""));

    let comment = use_resource(move || async move {
        get_comment(&GetCommentReq {
            comment_uuid: comment_uuid,
        })
        .await
    });

    let comment = &*comment.read();

    let result = match comment {
        Some(Ok(result)) => result.comment.clone(),
        _ => {
            return rsx! {
                span { "error fetching {comment_uuid}" }
            }
        }
    };

    let local_time = local_time(result.mtime);

    rsx! {
        div { class: "modal-body", grid_template_columns: "1fr",
            div {
                h3 { "Confirm comment deletion" }
                p { "Creator: {result.uid}, timestamp: {local_time}" }
                p { white_space: "pre", "{result.text}" }
                div { grid_template_columns: "1fr 1fr",
                    button {
                        onclick: move |_| async move {
                            let mut status_signal = status_signal;
                            let result = match delete_comment(
                                    &DeleteCommentReq {
                                        comment_uuid: comment_uuid,
                                    },
                                )
                                .await
                            {
                                Ok(_) => {
                                    MODAL_STACK.with_mut(|v| v.pop());
                                    return;
                                }
                                Err(err) => format!("Error deleting comment: {}", err.to_string()),
                            };
                            status_signal.set(result)
                        },
                        "Delete comment"
                    }
                    button {
                        onclick: move |_| {
                            MODAL_STACK.with_mut(|v| v.pop());
                        },
                        "Cancel"
                    }
                }
            }
        }
        div { class: "modal-footer",
            span { "{status_signal()}" }
        }
    }
}
