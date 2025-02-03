use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::{
    common::{
        local_time, modal::{Modal, MODAL_STACK}, storage::try_and_forget_local_storage, style
    }, gallery::GALLERY_ALBUM_KEY, Route
};
use api::{album::*, comment::*};

#[derive(Clone, PartialEq, Props)]
struct AlbumTableRowProps {
    album_uuid: AlbumUuid,
}

#[component]
fn AlbumTableRow(props: AlbumTableRowProps) -> Element {
    let album_uuid = props.album_uuid;

    let album = use_resource(move || async move {
        get_album(&GetAlbumReq {
            album_uuid: album_uuid,
        })
        .await
    });

    let album = &*album.read();

    let result = match album {
        Some(Ok(result)) => result.album.clone(),
        _ => {
            return rsx! {
                tr {
                    span { "error fetching {album_uuid}" }
                }
            }
        }
    };

    rsx! {
        tr {
            td {
                Link {
                    to: Route::AlbumDetail {
                        album_uuid: album_uuid.to_string(),
                    },
                    span { "{result.name}" }
                }
            }
            td { "{result.uid}" }
            td { "{result.gid}" }
            td { "REMOVE" }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
struct AlbumTableProps {
    albums: Vec<AlbumUuid>,
}

#[component]
fn AlbumTable(props: AlbumTableProps) -> Element {
    rsx! {
        div { style: "{style::TABLE}",
            table {
                tr {
                    th { "Name" }
                    th { "Creator" }
                    th { "Group" }
                    th { "Operations" }
                }

                for album_uuid in props.albums.iter() {
                    AlbumTableRow { album_uuid: *album_uuid }
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
struct CommentRowProps {
    comment_uuid: CommentUuid,
}

#[component]
fn CommentTableRow(props: CommentRowProps) -> Element {
    let comment_uuid = props.comment_uuid;

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
                tr {
                    span { "error fetching {comment_uuid}" }
                }
            }
        }
    };

    let local_time = local_time(result.mtime);

    rsx! {
        tr {
            td { "{result.uid}" }
            td { "{local_time}" }
            td {
                button {
                    float: "right",
                    onclick: move |_| async move {
                        MODAL_STACK.with_mut(|v| v.push(Modal::DeleteComment(comment_uuid)));
                    },
                    "Delete"
                }
            }
        }
        tr {
            td { colspan: 3, white_space: "pre", "{result.text}" }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
struct CommentTableProps {
    comments: Vec<CommentUuid>,
}

#[component]
fn CommentTable(props: CommentTableProps) -> Element {
    rsx! {
        div {
            style { "{style::TABLE}" }
            table {
                tr {
                    th { "User" }
                    th { "Timestamp" }
                    th { "Operations" }
                }

                for comment_uuid in props.comments.iter() {
                    CommentTableRow { comment_uuid: *comment_uuid }
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct MediaRelatedProps {
    albums: Vec<AlbumUuid>,
    comments: Vec<CommentUuid>,
}

#[component]
pub fn MediaRelated(props: MediaRelatedProps) -> Element {
    let albums = props.albums;
    let comments = props.comments;

    let album_highlight = try_and_forget_local_storage::<String>(GALLERY_ALBUM_KEY);

    rsx! {
        div { class: "gallery-related",
            span { "Albums {album_highlight}" }
            AlbumTable { albums }
            span { "Comments" }
            CommentTable { comments }
        }
    }
}
