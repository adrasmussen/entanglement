use std::{collections::HashSet, sync::Arc};

use anyhow;
use axum::{
    extract::{Extension, Json, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use tokio::sync::Mutex;
use tracing::instrument;

use crate::{
    auth::{check::AuthCheck, msg::AuthMsg},
    db::msg::DbMsg,
    http::{AppError, auth::CurrentUser, svc::HttpEndpoint},
    task::msg::TaskMsg,
};
use api::{auth::*, collection::*, comment::*, library::*, media::*, search::*, task::*};

// http api endpoints
//
// even the most casual observer will notice that the http endpoints
// are one-to-one (but not onto) the database service messages, which
// begs the question -- why introduce this extra layer?
//
// first, not all of the rust db crates are async safe, and trying to
// cram them into the axum Router state doesn't always work
//
// as a corrollary, this means that we don't need to worry about the
// database service details, as they are hidden behind the ESM layer
//
// second, it is the http service's job to enforce the correct auth
// policy for each of its endpoints; the db service just makes edits
// with none of the policy logic attached.  crucially, this includes
// clearing the access cache when collection contents are changed

// auth handlers
#[instrument(skip_all)]
pub(super) async fn get_users_in_group(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(_current_user): Extension<CurrentUser>,
    Json(message): Json<GetUsersInGroupReq>,
) -> Result<Response, AppError> {
    let state = state.clone();

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .auth_svc_sender
        .send(
            AuthMsg::UsersInGroup {
                resp: tx,
                gid: message.gid,
            }
            .into(),
        )
        .await?;

    let result = rx.await??;

    Ok(Json(GetUsersInGroupResp { uids: result }).into_response())
}

// media handlers
#[instrument(skip_all)]
pub(super) async fn get_media(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<GetMediaReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    if !state.can_access_media(&uid, &message.media_uuid).await? {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::GetMedia {
                resp: tx,
                media_uuid: message.media_uuid,
            }
            .into(),
        )
        .await?;

    let result = rx
        .await??
        .ok_or_else(|| anyhow::Error::msg("unknown media_uuid"))?;

    Ok(Json(GetMediaResp {
        media: result.0,
        collections: result.1,
        comments: result.2,
    })
    .into_response())
}

#[instrument(skip_all)]
pub(super) async fn update_media(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<UpdateMediaReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    if !state.owns_media(&uid, &message.media_uuid).await? {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::UpdateMedia {
                resp: tx,
                media_uuid: message.media_uuid,
                update: message.update,
            }
            .into(),
        )
        .await?;

    rx.await??;

    Ok(Json(UpdateMediaResp {}).into_response())
}

#[instrument(skip_all)]
pub(super) async fn search_media(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<SearchMediaReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    // auth handled as part of the db search

    let gid = state.groups_for_user(&uid).await?;

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::SearchMedia {
                resp: tx,
                gid,
                filter: message.filter,
            }
            .into(),
        )
        .await?;

    let result = rx.await??;

    Ok(Json(SearchMediaResp { media: result }).into_response())
}

#[instrument(skip_all)]
pub(super) async fn similar_media(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<SimilarMediaReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    // auth handled as part of the db search

    let gid = state.groups_for_user(&uid).await?;

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::SimilarMedia {
                resp: tx,
                gid,
                media_uuid: message.media_uuid,
                distance: message.distance,
            }
            .into(),
        )
        .await?;

    let result = rx.await??;

    Ok(Json(SimilarMediaResp { media: result }).into_response())
}

#[instrument(skip_all)]
pub(super) async fn add_comment(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<AddCommentReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    if !state
        .can_access_media(&uid, &message.comment.media_uuid)
        .await?
    {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::AddComment {
                resp: tx,
                comment: Comment {
                    media_uuid: message.comment.media_uuid,
                    mtime: 0,
                    uid,
                    text: message.comment.text,
                },
            }
            .into(),
        )
        .await?;

    let result = rx.await??;

    Ok(Json(AddCommentResp {
        comment_uuid: result,
    })
    .into_response())
}

#[instrument(skip_all)]
pub(super) async fn get_comment(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<GetCommentReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    if !state
        .can_access_comment(&uid, &message.comment_uuid)
        .await?
    {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::GetComment {
                resp: tx,
                comment_uuid: message.comment_uuid,
            }
            .into(),
        )
        .await?;

    let result = rx
        .await??
        .ok_or_else(|| anyhow::Error::msg("unknown comment_uuid"))?;

    Ok(Json(GetCommentResp { comment: result }).into_response())
}

#[instrument(skip_all)]
pub(super) async fn delete_comment(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<DeleteCommentReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    if !state.owns_comment(&uid, &message.comment_uuid).await? {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::DeleteComment {
                resp: tx,
                comment_uuid: message.comment_uuid,
            }
            .into(),
        )
        .await?;

    rx.await??;

    Ok(Json(DeleteCommentResp {}).into_response())
}

#[instrument(skip_all)]
pub(super) async fn update_comment(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<UpdateCommentReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    if !state.owns_comment(&uid, &message.comment_uuid).await? {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::UpdateComment {
                resp: tx,
                comment_uuid: message.comment_uuid,
                text: message.text,
            }
            .into(),
        )
        .await?;

    rx.await??;

    Ok(Json(UpdateCommentResp {}).into_response())
}

#[instrument(skip_all)]
pub(super) async fn add_collection(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<AddCollectionReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    // anyone may create an collection, but they must be in the group of the collection they create
    if !state
        .is_group_member(&uid, HashSet::from([message.collection.gid.clone()]))
        .await?
    {
        return Err(anyhow::Error::msg("User must be a member of collection group").into());
    }

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::AddCollection {
                resp: tx,
                collection: Collection {
                    uid,
                    gid: message.collection.gid,
                    mtime: 0,
                    name: message.collection.name,
                    note: message.collection.note,
                    tags: message.collection.tags,
                    cover: message.collection.cover,
                },
            }
            .into(),
        )
        .await?;

    let result = rx.await??;

    Ok(Json(AddCollectionResp {
        collection_uuid: result,
    })
    .into_response())
}

#[instrument(skip_all)]
pub(super) async fn get_collection(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<GetCollectionReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    if !state
        .can_access_collection(&uid, &message.collection_uuid)
        .await?
    {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::GetCollection {
                resp: tx,
                collection_uuid: message.collection_uuid,
            }
            .into(),
        )
        .await?;

    let result = rx
        .await??
        .ok_or_else(|| anyhow::Error::msg("unknown collection_uuid"))?;

    Ok(Json(GetCollectionResp { collection: result }).into_response())
}

#[instrument(skip_all)]
pub(super) async fn delete_collection(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<DeleteCollectionReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    if !state
        .owns_collection(&uid, &message.collection_uuid)
        .await?
    {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::DeleteCollection {
                resp: tx,
                collection_uuid: message.collection_uuid,
            }
            .into(),
        )
        .await?;

    rx.await??;

    Ok(Json(DeleteCollectionResp {}).into_response())
}

#[instrument(skip_all)]
pub(super) async fn update_collection(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<UpdateCollectionReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    if !state
        .owns_collection(&uid, &message.collection_uuid)
        .await?
    {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::UpdateCollection {
                resp: tx,
                collection_uuid: message.collection_uuid,
                update: message.update,
            }
            .into(),
        )
        .await?;

    rx.await??;

    Ok(Json(UpdateCollectionResp {}).into_response())
}

#[instrument(skip_all)]
pub(super) async fn add_media_to_collection(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<AddMediaToCollectionReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    if !state.owns_media(&uid, &message.media_uuid).await? {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    if !state
        .can_access_collection(&uid, &message.collection_uuid)
        .await?
    {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::AddMediaToCollection {
                resp: tx,
                media_uuid: message.media_uuid,
                collection_uuid: message.collection_uuid,
            }
            .into(),
        )
        .await?;

    rx.await??;

    state
        .clear_access_cache(Vec::from(&[message.media_uuid]))
        .await?;

    Ok(Json(AddMediaToCollectionResp {}).into_response())
}

#[instrument(skip_all)]
pub(super) async fn rm_media_from_collection(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<RmMediaFromCollectionReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    if !(state
        .owns_collection(&uid, &message.collection_uuid)
        .await?
        || state.owns_media(&uid, &message.media_uuid).await?
            && state
                .can_access_collection(&uid, &message.collection_uuid)
                .await?)
    {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::RmMediaFromCollection {
                resp: tx,
                media_uuid: message.media_uuid,
                collection_uuid: message.collection_uuid,
            }
            .into(),
        )
        .await?;

    rx.await??;

    state
        .clear_access_cache(Vec::from(&[message.media_uuid]))
        .await?;

    Ok(Json(RmMediaFromCollectionResp {}).into_response())
}

#[instrument(skip_all)]
pub(super) async fn search_collections(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<SearchCollectionsReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    // auth handled in db search

    let gid = state.groups_for_user(&uid).await?;

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::SearchCollections {
                resp: tx,
                gid,
                filter: message.filter,
            }
            .into(),
        )
        .await?;

    let result = rx.await??;

    Ok(Json(SearchCollectionsResp {
        collections: result,
    })
    .into_response())
}

#[instrument(skip_all)]
pub(super) async fn search_media_in_collection(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<SearchMediaInCollectionReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    // auth handled in db search

    let gid = state.groups_for_user(&uid).await?;

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::SearchMediaInCollection {
                resp: tx,
                gid,
                collection_uuid: message.collection_uuid,
                filter: message.filter,
            }
            .into(),
        )
        .await?;

    let result = rx.await??;

    Ok(Json(SearchMediaInCollectionResp { media: result }).into_response())
}

#[instrument(skip_all)]
pub(super) async fn get_library(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<GetLibraryReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    if !state.owns_library(&uid, &message.library_uuid).await? {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::GetLibrary {
                resp: tx,
                library_uuid: message.library_uuid,
            }
            .into(),
        )
        .await?;

    let result = rx
        .await??
        .ok_or_else(|| anyhow::Error::msg("unknown library_uuid"))?;

    Ok(Json(GetLibraryResp { library: result }).into_response())
}

#[instrument(skip_all)]
pub(super) async fn search_libraries(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<SearchLibrariesReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    // auth handled as part of the db search

    let gid = state.groups_for_user(&uid).await?;

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::SearchLibraries {
                resp: tx,
                gid,
                filter: message.filter,
            }
            .into(),
        )
        .await?;

    let result = rx.await??;

    Ok(Json(SearchLibrariesResp { libraries: result }).into_response())
}

#[instrument(skip_all)]
pub(super) async fn search_media_in_library(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<SearchMediaInLibraryReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    // auth handled as part of the db search

    let gid = state.groups_for_user(&uid).await?;

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::SearchMediaInLibrary {
                resp: tx,
                gid,
                library_uuid: message.library_uuid,
                hidden: message.hidden,
                filter: message.filter,
            }
            .into(),
        )
        .await?;

    let result = rx.await??;

    Ok(Json(SearchMediaInLibraryResp { media: result }).into_response())
}

#[instrument(skip_all)]
pub(super) async fn start_task(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<StartTaskReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    if !state.owns_library(&uid, &message.library_uuid).await? {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .task_svc_sender
        .send(
            TaskMsg::StartTask {
                resp: tx,
                library: TaskLibrary::User {
                    library_uuid: message.library_uuid,
                },
                task_type: message.task_type,
                uid: TaskUid::User { uid },
            }
            .into(),
        )
        .await?;

    rx.await??;

    Ok(Json(StartTaskResp {}).into_response())
}

#[instrument(skip_all)]
pub(super) async fn stop_task(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<StopTaskReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    if !state.owns_library(&uid, &message.library_uuid).await? {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .task_svc_sender
        .send(
            TaskMsg::StopTask {
                resp: tx,
                library: TaskLibrary::User {
                    library_uuid: message.library_uuid,
                },
            }
            .into(),
        )
        .await?;

    rx.await??;

    Ok(Json(StopTaskResp {}).into_response())
}

#[instrument(skip_all)]
pub(super) async fn show_tasks(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<ShowTasksReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    match message.library {
        TaskLibrary::User { library_uuid } => {
            if !state.owns_library(&uid, &library_uuid).await? {
                return Ok(StatusCode::UNAUTHORIZED.into_response());
            }
        }
        TaskLibrary::System => {}
    }

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .task_svc_sender
        .send(
            TaskMsg::ShowTasks {
                resp: tx,
                library: message.library,
            }
            .into(),
        )
        .await?;

    let result = rx.await??;

    Ok(Json(ShowTasksResp { tasks: result }).into_response())
}

// see notes in api/search.rs
//
// there are probably a dozen ways to do this better, including moving logic
// into the database calls, avoiding an expensive copy at the end, and so on
//
// TODO -- look into streaming responses and lazy loading in the UI
#[instrument(skip_all)]
pub(super) async fn batch_search_and_sort(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<BatchSearchAndSortReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    let gid = state.groups_for_user(&uid).await?;

    let (tx, rx) = tokio::sync::oneshot::channel();

    match message.req {
        SearchRequest::Media(request) => {
            state
                .db_svc_sender
                .send(
                    DbMsg::SearchMedia {
                        resp: tx,
                        gid,
                        filter: request.filter,
                    }
                    .into(),
                )
                .await?;
        }
        SearchRequest::Collection(request) => {
            state
                .db_svc_sender
                .send(
                    DbMsg::SearchMediaInCollection {
                        resp: tx,
                        gid,
                        collection_uuid: request.collection_uuid,
                        filter: request.filter,
                    }
                    .into(),
                )
                .await?;
        }
        SearchRequest::Library(request) => {
            state
                .db_svc_sender
                .send(
                    DbMsg::SearchMediaInLibrary {
                        resp: tx,
                        gid,
                        library_uuid: request.library_uuid,
                        hidden: request.hidden,
                        filter: request.filter,
                    }
                    .into(),
                )
                .await?;
        }
    };

    let media_uuids = rx.await??;

    let out = Mutex::new(Vec::<SearchResponse>::new());

    for media_uuid in media_uuids {
        let (tx, rx) = tokio::sync::oneshot::channel();

        state
            .db_svc_sender
            .send(
                DbMsg::GetMedia {
                    resp: tx,
                    media_uuid,
                }
                .into(),
            )
            .await?;

        let media_data = rx
            .await??
            .ok_or_else(|| anyhow::Error::msg("unknown media_uuid"))?;

        let mut out = out.lock().await;

        out.push(SearchResponse {
            media_uuid,
            media: media_data.0,
            collections: media_data.1,
            comments: media_data.2,
        });
    }

    let out = out.lock().await;

    Ok(Json(BatchSearchAndSortResp {
        media: out.to_vec(),
    })
    .into_response())
}
