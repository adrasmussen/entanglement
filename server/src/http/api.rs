use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use anyhow;
use axum::{
    extract::{Extension, Json, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use futures::future::BoxFuture;
use tracing::{instrument, warn};

use crate::{
    auth::{check::AuthCheck, msg::AuthMsg},
    db::msg::DbMsg,
    http::{AppError, auth::CurrentUser, svc::HttpEndpoint},
    task::msg::TaskMsg,
};
use api::{
    auth::*,
    collection::*,
    comment::*,
    library::*,
    media::*,
    omni::*,
    page::PageMeta,
    page::PageRequest,
    query::{SearchClass, SearchKind, SearchQuery},
    search::*,
    slug::*,
    task::*,
};

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

#[instrument(skip_all)]
pub(super) async fn get_current_user(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(_message): Json<GetCurrentUserReq>,
) -> Result<Response, AppError> {
    let mut groups: Vec<String> = state
        .groups_for_user(&current_user.uid)
        .await?
        .into_iter()
        .collect();
    groups.sort();

    Ok(Json(GetCurrentUserResp {
        uid: current_user.uid,
        groups,
    })
    .into_response())
}

// media handlers
#[instrument(skip_all)]
pub(super) async fn get_media(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<GetMediaReq>,
) -> Result<Response, AppError> {
    if !state
        .can_access_media(&current_user.uid, &message.media_uuid)
        .await?
    {
        return Ok(StatusCode::NOT_FOUND.into_response());
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

    // only list the collections that the user's groups can see
    let gid = state.groups_for_user(&current_user.uid).await?;
    let visible = state.visible_collections(gid).await?;

    Ok(Json(GetMediaResp {
        media: result.0,
        collections: result
            .1
            .into_iter()
            .filter(|c| visible.contains(c))
            .collect(),
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
    if !state
        .owns_media(&current_user.uid, &message.media_uuid)
        .await?
    {
        return Ok(StatusCode::NOT_FOUND.into_response());
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

// a 400 for a structured search query that the media fields don't declare (an invalid
// query is the client's error, which AppError would report as a 500)
fn check_media_filter(filter: &SearchFilter) -> Result<(), Response> {
    filter
        .validate_media()
        .map_err(|msg| (StatusCode::BAD_REQUEST, msg).into_response())
}

// the same for the collection and library list searches
fn check_list_filter<C: SearchClass>(filter: &ListFilter) -> Result<(), Response> {
    filter
        .validate::<C>()
        .map_err(|msg| (StatusCode::BAD_REQUEST, msg).into_response())
}

#[instrument(skip_all)]
pub(super) async fn search_media(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<SearchMediaReq>,
) -> Result<Response, AppError> {
    if let Err(response) = check_media_filter(&message.opts.filter) {
        return Ok(response);
    }

    // auth handled as part of the db search

    let gid = state.groups_for_user(&current_user.uid).await?;

    let (tx, rx) = tokio::sync::oneshot::channel();

    let page_req = message.opts.page.clone();

    state
        .db_svc_sender
        .send(
            DbMsg::SearchMedia {
                resp: tx,
                gid,
                opts: message.opts,
            }
            .into(),
        )
        .await?;

    let result = rx.await??;

    Ok(Json(SearchMediaResp {
        meta: result.meta(&page_req),
        media: result.items,
        total: result.total,
    })
    .into_response())
}

#[instrument(skip_all)]
pub(super) async fn similar_media(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<SimilarMediaReq>,
) -> Result<Response, AppError> {
    // the results are filtered by the db search, but the source media must be checked here,
    // otherwise any media_uuid can be used to probe for similar items
    if !state
        .can_access_media(&current_user.uid, &message.media_uuid)
        .await?
    {
        return Ok(StatusCode::NOT_FOUND.into_response());
    }

    let gid = state.groups_for_user(&current_user.uid).await?;

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
    if !state
        .can_access_media(&current_user.uid, &message.comment.media_uuid)
        .await?
    {
        return Ok(StatusCode::NOT_FOUND.into_response());
    }

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::AddComment {
                resp: tx,
                comment: Comment {
                    media_uuid: message.comment.media_uuid,
                    uid: current_user.uid,
                    date: message.comment.date,
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
    if !state
        .can_access_comment(&current_user.uid, &message.comment_uuid)
        .await?
    {
        return Ok(StatusCode::NOT_FOUND.into_response());
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
    if !state
        .owns_comment(&current_user.uid, &message.comment_uuid)
        .await?
    {
        return Ok(StatusCode::NOT_FOUND.into_response());
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
    if !state
        .owns_comment(&current_user.uid, &message.comment_uuid)
        .await?
    {
        return Ok(StatusCode::NOT_FOUND.into_response());
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
    // anyone may create an collection, but they must be in the group of the collection they create
    if !state
        .is_group_member(
            &current_user.uid,
            HashSet::from([message.collection.gid.clone()]),
        )
        .await?
    {
        return Err(anyhow::Error::msg("User must be a member of collection group").into());
    }

    // the cover must be media the creator can see; otherwise it's answered like any unknown id
    if let Some(cover) = &message.collection.cover
        && !state.can_access_media(&current_user.uid, cover).await?
    {
        return Ok(StatusCode::NOT_FOUND.into_response());
    }

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::AddCollection {
                resp: tx,
                collection: Collection {
                    uid: current_user.uid,
                    gid: message.collection.gid,
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
    if !state
        .can_access_collection(&current_user.uid, &message.collection_uuid)
        .await?
    {
        return Ok(StatusCode::NOT_FOUND.into_response());
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
    if !state
        .owns_collection(&current_user.uid, &message.collection_uuid)
        .await?
    {
        return Ok(StatusCode::NOT_FOUND.into_response());
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
    if !state
        .owns_collection(&current_user.uid, &message.collection_uuid)
        .await?
    {
        return Ok(StatusCode::NOT_FOUND.into_response());
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
    if !state
        .owns_media(&current_user.uid, &message.media_uuid)
        .await?
    {
        return Ok(StatusCode::NOT_FOUND.into_response());
    }

    if !state
        .can_access_collection(&current_user.uid, &message.collection_uuid)
        .await?
    {
        return Ok(StatusCode::NOT_FOUND.into_response());
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
    if !(state
        .owns_collection(&current_user.uid, &message.collection_uuid)
        .await?
        || state
            .owns_media(&current_user.uid, &message.media_uuid)
            .await?
            && state
                .can_access_collection(&current_user.uid, &message.collection_uuid)
                .await?)
    {
        return Ok(StatusCode::NOT_FOUND.into_response());
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
    let filter = ListFilter::from_req(message.filter, message.query);

    if let Err(response) = check_list_filter::<Collection>(&filter) {
        return Ok(response);
    }

    // auth handled in db search

    let gid = state.groups_for_user(&current_user.uid).await?;

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::SearchCollections {
                resp: tx,
                gid,
                filter,
                // uuid-only like the other search endpoints, so None (used by the
                // collection pickers) is left unbounded; the collections page pages itself
                page: message.page.clone(),
            }
            .into(),
        )
        .await?;

    let result = rx.await??;

    Ok(Json(SearchCollectionsResp {
        meta: result.meta(&message.page),
        collections: result.items,
        total: result.total,
    })
    .into_response())
}

#[instrument(skip_all)]
pub(super) async fn search_media_in_collection(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<SearchMediaInCollectionReq>,
) -> Result<Response, AppError> {
    if let Err(response) = check_media_filter(&message.opts.filter) {
        return Ok(response);
    }

    // auth handled in db search

    let gid = state.groups_for_user(&current_user.uid).await?;

    let (tx, rx) = tokio::sync::oneshot::channel();

    let page_req = message.opts.page.clone();

    state
        .db_svc_sender
        .send(
            DbMsg::SearchMediaInCollection {
                resp: tx,
                gid,
                collection_uuid: message.collection_uuid,
                opts: message.opts,
            }
            .into(),
        )
        .await?;

    let result = rx.await??;

    Ok(Json(SearchMediaInCollectionResp {
        meta: result.meta(&page_req),
        media: result.items,
        total: result.total,
    })
    .into_response())
}

#[instrument(skip_all)]
pub(super) async fn get_library(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<GetLibraryReq>,
) -> Result<Response, AppError> {
    if !state
        .owns_library(&current_user.uid, &message.library_uuid)
        .await?
    {
        return Ok(StatusCode::NOT_FOUND.into_response());
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
    let filter = ListFilter::from_req(message.filter, message.query);

    if let Err(response) = check_list_filter::<Library>(&filter) {
        return Ok(response);
    }

    // auth handled as part of the db search

    let gid = state.groups_for_user(&current_user.uid).await?;

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::SearchLibraries {
                resp: tx,
                gid,
                filter,
                page: message.page.clone(),
            }
            .into(),
        )
        .await?;

    let result = rx.await??;

    Ok(Json(SearchLibrariesResp {
        meta: result.meta(&message.page),
        libraries: result.items,
    })
    .into_response())
}

#[instrument(skip_all)]
pub(super) async fn search_media_in_library(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<SearchMediaInLibraryReq>,
) -> Result<Response, AppError> {
    if let Err(response) = check_media_filter(&message.opts.filter) {
        return Ok(response);
    }

    // auth handled as part of the db search

    let gid = state.groups_for_user(&current_user.uid).await?;

    let (tx, rx) = tokio::sync::oneshot::channel();

    let page_req = message.opts.page.clone();

    state
        .db_svc_sender
        .send(
            DbMsg::SearchMediaInLibrary {
                resp: tx,
                gid,
                library_uuid: message.library_uuid,
                hidden: message.hidden,
                opts: message.opts,
            }
            .into(),
        )
        .await?;

    let result = rx.await??;

    Ok(Json(SearchMediaInLibraryResp {
        meta: result.meta(&page_req),
        media: result.items,
        total: result.total,
    })
    .into_response())
}

// omnisearch: every class searched with the same text, each as its own concurrent query
// with its own timeout (see api/src/omni.rs).  each runs the class's ordinary gid-filtered
// search, the same statement as its local page, so the counts and hits here are exactly
// what that page shows for the same ?q=, and nothing outside the user's groups can appear
#[instrument(skip_all)]
pub(super) async fn omni_search(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<OmniSearchReq>,
) -> Result<Response, AppError> {
    let plan = match message.plan() {
        Ok(plan) => plan,
        Err(err) => return Ok((StatusCode::BAD_REQUEST, err).into_response()),
    };

    // auth handled as part of the db searches
    let gid = state.groups_for_user(&current_user.uid).await?;
    let page = PageRequest::first(u64::from(plan.per_group), true);

    let groups = futures::future::join_all(plan.groups.into_iter().map(|(spec, group_plan)| {
        let state = state.clone();
        let gid = gid.clone();
        let page = page.clone();
        async move {
            let mut group = HitGroup {
                kind: spec.kind,
                label: spec.label.to_string(),
                status: GroupStatus::Ok,
                hits: Vec::new(),
                meta: PageMeta::default(),
            };

            let query = match group_plan {
                GroupPlan::NotApplicable { field } => {
                    group.status = GroupStatus::NotApplicable { field };
                    return group;
                }
                GroupPlan::Search(query) => query,
            };

            let run = omni_provider(&state, spec.kind, gid, query, page);
            match tokio::time::timeout(Duration::from_millis(PROVIDER_TIMEOUT_MS), run).await {
                Ok(Ok((hits, meta))) => {
                    group.hits = hits;
                    group.meta = meta;
                }
                Ok(Err(err)) => {
                    // the detail stays in the log; the group just says it failed
                    warn!({ kind = ?spec.kind, error = %err }, "omnisearch provider failed");
                    group.status = GroupStatus::Error("search failed".to_string());
                }
                Err(_) => {
                    warn!({ kind = ?spec.kind }, "omnisearch provider timed out");
                    group.status = GroupStatus::TimedOut;
                }
            }

            group
        }
    }))
    .await;

    Ok(Json(OmniSearchResp { groups }).into_response())
}

// one provider: its local search for the first page with the exact total, then the hits'
// titles.  hydration only ever looks up the uuids the gid-filtered search returned
async fn omni_provider(
    state: &Arc<HttpEndpoint>,
    kind: SearchKind,
    gid: HashSet<String>,
    query: SearchQuery,
    page: PageRequest,
) -> anyhow::Result<(Vec<SearchHit>, PageMeta)> {
    macro_rules! ask {
        ($msg:ident { $($field:ident : $val:expr),* $(,)? }) => {{
            let (tx, rx) = tokio::sync::oneshot::channel();
            state
                .db_svc_sender
                .send(DbMsg::$msg { resp: tx, $($field: $val),* }.into())
                .await?;
            rx.await??
        }};
    }

    match kind {
        SearchKind::Media => {
            let found = ask!(SearchMedia {
                gid: gid,
                opts: SearchOptions {
                    filter: SearchFilter::Query(query),
                    page: page.clone(),
                    ..Default::default()
                },
            });
            let meta = found.meta(&page);
            if found.items.is_empty() {
                return Ok((Vec::new(), meta));
            }
            let order: HashMap<MediaUuid, usize> = found
                .items
                .iter()
                .enumerate()
                .map(|(i, u)| (*u, i))
                .collect();
            let mut details = ask!(GetMediaBatch {
                media_uuids: found.items.clone(),
            });
            // keep the search's order; media deleted in between is skipped
            details.sort_by_key(|d| order.get(&d.media_uuid).copied().unwrap_or(usize::MAX));
            let hits = details
                .into_iter()
                .map(|d| SearchHit {
                    id: HitId::Media(d.media_uuid),
                    title: file_name(&d.media.path).to_string(),
                    subtitle: media_subtitle(&d.media.date, &d.media.note),
                    thumbnail: Some(d.media_uuid),
                })
                .collect();
            Ok((hits, meta))
        }
        SearchKind::Collection => {
            let found = ask!(SearchCollections {
                gid: gid,
                filter: ListFilter::Query(query),
                page: page.clone(),
            });
            let meta = found.meta(&page);
            let mut hits = Vec::with_capacity(found.items.len());
            for collection_uuid in found.items {
                if let Some(c) = ask!(GetCollection {
                    collection_uuid: collection_uuid
                }) {
                    hits.push(SearchHit {
                        id: HitId::Collection(collection_uuid),
                        title: c.name,
                        subtitle: c.note,
                        thumbnail: c.cover,
                    });
                }
            }
            Ok((hits, meta))
        }
        SearchKind::Library => {
            let found = ask!(SearchLibraries {
                gid: gid,
                filter: ListFilter::Query(query),
                page: page.clone(),
            });
            let meta = found.meta(&page);
            let mut hits = Vec::with_capacity(found.items.len());
            for library_uuid in found.items {
                if let Some(l) = ask!(GetLibrary {
                    library_uuid: library_uuid
                }) {
                    hits.push(SearchHit {
                        id: HitId::Library(library_uuid),
                        title: l.path,
                        subtitle: String::new(),
                        thumbnail: None,
                    });
                }
            }
            Ok((hits, meta))
        }
    }
}

// the values a field's select offers.  static values come straight from the field metadata;
// tags are the distinct tags of the records the user can see, counted over those records
// only, so nothing about hidden or other groups' records leaks through the filter options
#[instrument(skip_all)]
pub(super) async fn get_slugs(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<GetSlugsReq>,
) -> Result<Response, AppError> {
    let source = match message.source() {
        Ok(source) => source,
        Err(err) => return Ok((StatusCode::BAD_REQUEST, err).into_response()),
    };

    let slugs = match source {
        SlugSource::Static(slugs) => slugs,
        SlugSource::Tags {
            kind,
            prefix,
            limit,
        } => {
            // auth handled as part of the db query
            let gid = state.groups_for_user(&current_user.uid).await?;

            let (tx, rx) = tokio::sync::oneshot::channel();
            state
                .db_svc_sender
                .send(
                    DbMsg::TagSlugs {
                        resp: tx,
                        gid,
                        kind,
                        prefix,
                        limit,
                    }
                    .into(),
                )
                .await?;
            rx.await??
        }
    };

    Ok(Json(GetSlugsResp { slugs }).into_response())
}

// counts are the totals of the ordinary searches with an empty filter and a count-only
// page, so they run exactly the same gid-filtered queries as the lists and the two can
// never disagree.  every scope is sent before any is awaited, so they run concurrently
#[instrument(skip_all)]
pub(super) async fn get_counts(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<GetCountsReq>,
) -> Result<Response, AppError> {
    if message.scopes.len() > MAX_COUNT_SCOPES {
        return Ok((
            StatusCode::BAD_REQUEST,
            format!("at most {MAX_COUNT_SCOPES} count scopes per request"),
        )
            .into_response());
    }

    // auth handled as part of the db searches
    let gid = state.groups_for_user(&current_user.uid).await?;

    let opts = || SearchOptions {
        page: PageRequest::count_only(),
        ..Default::default()
    };

    let mut pending: Vec<BoxFuture<'static, anyhow::Result<Option<u64>>>> = Vec::new();

    // each use sends its message and queues the receiver; the SearchPage item types
    // differ, so only the total is kept
    macro_rules! send {
        ($msg:ident { $($field:ident : $val:expr),* $(,)? }) => {{
            let (tx, rx) = tokio::sync::oneshot::channel();
            state
                .db_svc_sender
                .send(DbMsg::$msg { resp: tx, $($field: $val),* }.into())
                .await?;
            pending.push(Box::pin(async move { Ok(rx.await??.total) }));
        }};
    }

    for scope in &message.scopes {
        match scope {
            CountScope::Media => send!(SearchMedia {
                gid: gid.clone(),
                opts: opts(),
            }),
            CountScope::Collections => send!(SearchCollections {
                gid: gid.clone(),
                filter: ListFilter::Literal(String::new()),
                page: PageRequest::count_only(),
            }),
            CountScope::Libraries => send!(SearchLibraries {
                gid: gid.clone(),
                filter: ListFilter::Literal(String::new()),
                page: PageRequest::count_only(),
            }),
            CountScope::MediaInCollection { collection_uuid } => {
                send!(SearchMediaInCollection {
                    gid: gid.clone(),
                    collection_uuid: *collection_uuid,
                    opts: opts(),
                })
            }
            CountScope::MediaInLibrary {
                library_uuid,
                hidden,
            } => send!(SearchMediaInLibrary {
                gid: gid.clone(),
                library_uuid: *library_uuid,
                hidden: *hidden,
                opts: opts(),
            }),
        }
    }

    let totals = futures::future::try_join_all(pending).await?;

    let counts = message
        .scopes
        .into_iter()
        .zip(totals)
        .map(|(scope, total)| {
            total
                .map(|count| ScopeCount { scope, count })
                .ok_or_else(|| anyhow::Error::msg("count response is missing the total"))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(Json(GetCountsResp { counts }).into_response())
}

#[instrument(skip_all)]
pub(super) async fn start_task(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<StartTaskReq>,
) -> Result<Response, AppError> {
    if !state
        .owns_library(&current_user.uid, &message.library_uuid)
        .await?
    {
        return Ok(StatusCode::NOT_FOUND.into_response());
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
                uid: TaskUid::User {
                    uid: current_user.uid,
                },
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
    if !state
        .owns_library(&current_user.uid, &message.library_uuid)
        .await?
    {
        return Ok(StatusCode::NOT_FOUND.into_response());
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
    match message.library {
        TaskLibrary::User { library_uuid } => {
            if !state.owns_library(&current_user.uid, &library_uuid).await? {
                return Ok(StatusCode::NOT_FOUND.into_response());
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
// the page is selected in sql (limit/offset from the request, clamped to MAX_PAGE), and
// then hydrated with a single batched lookup rather than one GetMedia round trip per item
#[instrument(skip_all)]
pub(super) async fn batch_search_and_sort(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<BatchSearchAndSortReq>,
) -> Result<Response, AppError> {
    if let Err(response) = check_media_filter(&message.req.opts().filter) {
        return Ok(response);
    }

    let gid = state.groups_for_user(&current_user.uid).await?;

    let mut req = message.req;

    {
        let opts = req.opts_mut();
        opts.page.limit = Some(opts.page.limit.unwrap_or(MAX_PAGE).min(MAX_PAGE));
    }

    // the request as it will be run, i.e. after the clamp, so meta.limit echoes the limit
    // actually used
    let page_req = req.opts().page.clone();

    // collections attached to each media record are trimmed to the ones these groups can see
    let visible = state.visible_collections(gid.clone()).await?;

    let (tx, rx) = tokio::sync::oneshot::channel();

    let msg = match req {
        SearchRequest::Media(request) => DbMsg::SearchMedia {
            resp: tx,
            gid,
            opts: request.opts,
        },
        SearchRequest::Collection(request) => DbMsg::SearchMediaInCollection {
            resp: tx,
            gid,
            collection_uuid: request.collection_uuid,
            opts: request.opts,
        },
        SearchRequest::Library(request) => DbMsg::SearchMediaInLibrary {
            resp: tx,
            gid,
            library_uuid: request.library_uuid,
            hidden: request.hidden,
            opts: request.opts,
        },
    };

    state.db_svc_sender.send(msg.into()).await?;

    let page = rx.await??;
    let page_len = page.items.len() as u64;
    let meta = page.meta(&page_req);

    // past the end of the results there is nothing to hydrate
    if page.items.is_empty() {
        return Ok(Json(BatchSearchAndSortResp {
            media: Vec::new(),
            total: page.total,
            page_len: Some(0),
            meta,
        })
        .into_response());
    }

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::GetMediaBatch {
                resp: tx,
                media_uuids: page.items,
            }
            .into(),
        )
        .await?;

    // media deleted between the two queries is simply skipped, which is why the client is
    // told the length of the page before hydration
    let media = rx
        .await??
        .into_iter()
        .map(|d| SearchResponse {
            media_uuid: d.media_uuid,
            media: d.media,
            collections: d
                .collections
                .into_iter()
                .filter(|c| visible.contains(c))
                .collect(),
            comments: d.comments,
        })
        .collect();

    Ok(Json(BatchSearchAndSortResp {
        media,
        total: page.total,
        page_len: Some(page_len),
        meta,
    })
    .into_response())
}
