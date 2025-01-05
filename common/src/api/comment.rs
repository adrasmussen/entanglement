use serde::{Deserialize, Serialize};

use crate::api::media::MediaUuid;
use crate::endpoint;

// structs and types

pub type CommentUuid = i64;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Comment {
    pub media_uuid: MediaUuid,
    pub ctime: i64,
    pub uid: String,
    pub text: String,
}

// messages

// add a comment to media
endpoint!(AddComment);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddCommentReq {
    pub media_uuid: MediaUuid,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddCommentResp {
    pub comment_uuid: CommentUuid,
}

// fetch comments for media
endpoint!(GetComment);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetCommentReq {
    pub comment_uuid: CommentUuid
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetCommentResp {
    pub comment: Comment,
}

// update a comment
//
// empty string (instead of None) removes the comment
endpoint!(UpdateComment);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateCommentReq {
    pub comment_uuid: CommentUuid,
    pub text: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateCommentResp {}
