use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::{
    common::{
        local_time,
        modal::{Modal, MODAL_STACK},
        storage::try_and_forget_local_storage,
        style,
    },
    gallery::GALLERY_ALBUM_KEY,
    Route,
};
use api::{album::*, comment::*, media::MediaUuid};

#[derive(Clone, PartialEq, Props)]
struct AlbumTableRowProps {
    media_uuid: MediaUuid,
    album_uuid: AlbumUuid,
}

#[component]
fn AlbumTableRow(props: AlbumTableRowProps) -> Element {
    let media_uuid = props.media_uuid;
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
            td {
                button {
                    float: "right",
                    onclick: move |_| async move {
                        MODAL_STACK
                            .with_mut(|v| v.push(Modal::RmMediaFromAlbum(media_uuid, album_uuid)));
                    },
                    "Remove"
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
struct AlbumTableProps {
    media_uuid: MediaUuid,
    albums: Vec<AlbumUuid>,
}

#[component]
fn AlbumTable(props: AlbumTableProps) -> Element {
    let media_uuid = props.media_uuid;
    let albums = props.albums;

    rsx! {
        div { style: "{style::TABLE}",
            table {
                tr {
                    th { "Name" }
                    th { "Creator" }
                    th { "Group" }
                    th { "Operations" }
                }

                for album_uuid in albums.iter() {
                    AlbumTableRow {
                        key: "{album_uuid}",
                        media_uuid,
                        album_uuid: *album_uuid,
                    }
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

    tracing::debug!("rendering comment {comment_uuid}");

    let comment = use_resource(move || async move {
        tracing::debug!("running get_comment for {comment_uuid}");

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

// TODO -- this does not render correctly after deleting a comment in the middle
// of the vector, and attempts to signalize it haven't been particularly successful
//
// oddly enough, it follows the same basic logic as the MediaGrid in this module,
// so it's not clear why the length of the vec is correct but the contents are not
//
// we should also sort the coments by timestamp since Uuid is not necessarily stable
#[component]
fn CommentTable(props: CommentTableProps) -> Element {
    let comments = props.comments;
    tracing::debug!({comments = ?comments}, "found comments");

    rsx! {
        div {
            style { "{style::TABLE}" }
            table {
                tr {
                    th { "User" }
                    th { "Timestamp" }
                    th { "Operations" }
                }

                for comment_uuid in comments.iter() {
                    CommentTableRow { key: "{comment_uuid}", comment_uuid: *comment_uuid }
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct MediaRelatedProps {
    update_signal: Signal<()>,
    media_uuid: MediaUuid,
    albums: Vec<AlbumUuid>,
    comments: Vec<CommentUuid>,
}

#[component]
pub fn MediaRelated(props: MediaRelatedProps) -> Element {
    let media_uuid = props.media_uuid;
    let albums = props.albums;
    let comments = props.comments;

    let album_highlight = try_and_forget_local_storage::<String>(GALLERY_ALBUM_KEY);

    rsx! {
        div { class: "gallery-related",
            div {
                h4 { "Albums {album_highlight}" }
                AlbumTable { media_uuid, albums }
            }
            div {
                h4 { "Comments" }
                CommentTable { comments }
            }
            div {
                h4 { "Similar media" }
                span { "not implemented {media_uuid}" }
            }
        }
    }
}
