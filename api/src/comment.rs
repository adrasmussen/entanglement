use serde::{Deserialize, Serialize};

use crate::{endpoint, media::MediaUuid};

// structs and types

pub type CommentUuid = u64;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Comment {
    pub media_uuid: MediaUuid,
    pub mtime: i64,
    pub uid: String,
    pub text: String,
}

// messages

// add a comment to media
//
// note that this exposes an awkward abstraction layer violation:
// the uid and timestamp both are determined by the http service,
// and so are ignored by this endpoint
endpoint!(AddComment);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AddCommentReq {
    pub comment: Comment,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AddCommentResp {
    pub comment_uuid: CommentUuid,
}

// fetch comments for media
endpoint!(GetComment);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GetCommentReq {
    pub comment_uuid: CommentUuid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GetCommentResp {
    pub comment: Comment,
}

// delete a comment
endpoint!(DeleteComment);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DeleteCommentReq {
    pub comment_uuid: CommentUuid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DeleteCommentResp {}

// update a comment
endpoint!(UpdateComment);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UpdateCommentReq {
    pub comment_uuid: CommentUuid,
    pub text: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UpdateCommentResp {}
