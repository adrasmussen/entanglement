use std::collections::HashSet;
use std::net::{SocketAddr, SocketAddrV6};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow;
use async_cell::sync::AsyncCell;
use async_trait::async_trait;
use axum::{
    extract::{Extension, Json, Path, Request, State},
    http::StatusCode,
    middleware,
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Router,
};
use chrono::Local;
use tokio::sync::Mutex;
use tokio_util::io::ReaderStream;
use tower::Service;
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use tracing::{debug, error, info, instrument};

use crate::{
    auth::{check::AuthCheck, msg::AuthMsg},
    db::msg::DbMsg,
    http::{
        auth::{proxy_auth, CurrentUser},
        AppError,
    },
    service::{
        ESInner, ESMReceiver, ESMRegistry, ESMSender, EntanglementService, ServiceType, ESM,
    },
    task::msg::TaskMsg,
};
use api::{auth::*, collection::*, comment::*, library::*, media::*, task::*};
use common::config::ESConfig;

// http service
//
// the http service handles both api calls and streaming media, along
// with enforcing the authentication policies on those endpoints
//
// hyper and axum form the core of the service, and the outer struct
// collects the hyper handle alongside the ESM handle

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
pub struct HttpService {
    config: Arc<ESConfig>,
    receiver: Arc<Mutex<ESMReceiver>>,
    msg_handle: AsyncCell<tokio::task::JoinHandle<anyhow::Result<()>>>,
    hyper_handle: AsyncCell<tokio::task::JoinHandle<anyhow::Result<()>>>,
}

#[async_trait]
impl EntanglementService for HttpService {
    type Inner = HttpEndpoint;

    fn create(config: Arc<ESConfig>, registry: &ESMRegistry) -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel::<ESM>(1024);

        registry
            .insert(ServiceType::Http, tx)
            .expect("failed to add http sender to registry");

        HttpService {
            config: config.clone(),
            receiver: Arc::new(Mutex::new(rx)),
            msg_handle: AsyncCell::new(),
            hyper_handle: AsyncCell::new(),
        }
    }

    #[instrument(skip(self, registry))]
    async fn start(&self, registry: &ESMRegistry) -> anyhow::Result<()> {
        info!("starting http service");

        let receiver = Arc::clone(&self.receiver);
        let state = Arc::new(HttpEndpoint::new(self.config.clone(), registry.clone())?);

        // if we wanted to support more socket types, we could spawn several listeners since they
        // don't have any relevant state and the message handlers are all fully concurrent
        let socket = SocketAddr::from(
            self.config
                .http_socket
                .parse::<SocketAddrV6>()
                .expect("Failed to parse http_socket ipv6 address/port"),
        );

        let hyper_handle = state.clone().serve_http(socket).await;
        self.hyper_handle.set(hyper_handle);

        let msg_serve = {
            async move {
                let mut receiver = receiver.lock().await;

                while let Some(msg) = receiver.recv().await {
                    let state = Arc::clone(&state);
                    tokio::task::spawn(async move {
                        match state.message_handler(msg).await {
                            Ok(()) => (),
                            Err(err) => {
                                error!({service = "http_service", channel = "esm", error = %err})
                            }
                        }
                    });
                }

                Err(anyhow::Error::msg(format!(
                    "http_service esm channel disconnected"
                )))
            }
        };

        let msg_handle = tokio::task::spawn(msg_serve);
        self.msg_handle.set(msg_handle);

        debug!("started http service");
        Ok(())
    }
}

// http endpoint
//
// this struct is the actual responder that binds to the socket and serves http
#[derive(Clone, Debug)]
pub struct HttpEndpoint {
    config: Arc<ESConfig>,
    registry: ESMRegistry,
    auth_svc_sender: ESMSender,
    db_svc_sender: ESMSender,
    task_svc_sender: ESMSender,
}

#[async_trait]
impl ESInner for HttpEndpoint {
    fn new(config: Arc<ESConfig>, registry: ESMRegistry) -> anyhow::Result<Self> {
        Ok(HttpEndpoint {
            config: config.clone(),
            registry: registry.clone(),
            // panic if we can't find all of the necessary senders, since this is a
            // compile-time problem and not a runtime problem
            auth_svc_sender: registry.get(&ServiceType::Auth).unwrap().clone(),
            db_svc_sender: registry.get(&ServiceType::Db).unwrap().clone(),
            task_svc_sender: registry.get(&ServiceType::Task).unwrap().clone(),
        })
    }

    fn registry(&self) -> ESMRegistry {
        self.registry.clone()
    }

    // currently there are no useful messages for this service to respond to
    async fn message_handler(&self, esm: ESM) -> anyhow::Result<()> {
        match esm {
            _ => Err(anyhow::Error::msg("not implemented")),
        }
    }
}

// http endpoint details
//
// the following functions contain all of the routing and responding behavior
// for the http server
//
// convenience functions are part of the impl block, but the routing functions
// need to depend on State<_> instead of Self<_> and so live outside
impl HttpEndpoint {
    // we use axum to construct the router from three pieces, and then feed it manually
    // into hyper's service_fn so that we have maximum control over the binding process
    //
    // specifically axum::serve() doesn't really play nice with TLS, which we'll want
    // to offer as an option eventually
    #[instrument(skip_all)]
    async fn serve_http(
        self: Arc<Self>,
        socket: SocketAddr,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        info!("starting axum/hyper http listener");

        let config = self.config.clone();
        let state = Arc::clone(&self);

        // app -- the WASM webapp

        // this controls which route actually serves the app
        //
        // if running behind a reverse proxy, this URL can be configured (as well as the
        // frontend setting in dioxsus.toml)
        let app_url_root = config.http_url_root.clone();

        // this is the filesystem location of the /dist folder created by the dioxsus
        // build process
        //
        // it should be baked into a containerized runtime for the server, but does not
        // necessarily have to be so (it can be hosted outside)
        let app_web_dir = config.http_doc_root.clone();

        // the dioxsus build process creates a /dist folder after compiling the actual
        // app in /target, and we just grab the whole thing and serve it using tower's
        // built-in directory server
        //
        // using the fallback here is a bit tricky -- any file that can't be found will
        // instead go back to the root index, so be careful not to get into a loop with
        // the main fallback
        let app_router = Router::new().fallback_service(ServeDir::new(&app_web_dir).fallback(
            ServeFile::new(PathBuf::from(&app_web_dir).join("index.html")),
        ));

        // media -- streaming files to clients

        // we will likely need more routes here once we do more complicated things with videos
        let media_router = Router::new()
            .route("/{dir}/{media_uuid}", get(stream_media))
            .with_state(state.clone());

        // api -- the server's remote method calls

        // it would be nice to come up with a macro to automate some of this...
        let api_router: Router<()> = Router::new()
            .route("/GetUsersInGroup", post(get_users_in_group))
            .route("/GetMedia", post(get_media))
            .route("/UpdateMedia", post(update_media))
            .route("/SearchMedia", post(search_media))
            .route("/SimilarMedia", post(similar_media))
            .route("/AddComment", post(add_comment))
            .route("/GetComment", post(get_comment))
            .route("/DeleteComment", post(delete_comment))
            .route("/UpdateComment", post(update_comment))
            .route("/AddCollection", post(add_collection))
            .route("/GetCollection", post(get_collection))
            .route("/DeleteCollection", post(delete_collection))
            .route("/UpdateCollection", post(update_collection))
            .route("/AddMediaToCollection", post(add_media_to_collection))
            .route("/RmMediaFromCollection", post(rm_media_from_collection))
            .route("/SearchCollections", post(search_collections))
            .route("/SearchMediaInCollection", post(search_media_in_collection))
            .route("/GetLibrary", post(get_library))
            .route("/SearchLibraries", post(search_libraries))
            .route("/SearchMediaInLibrary", post(search_media_in_library))
            .route("/StartTask", post(start_task))
            .route("/StopTask", post(stop_task))
            .route("/ShowTasks", post(show_tasks))
            .with_state(state.clone());

        // combine the routes (note that this can panic if the routes overlap) and add any relevant
        // middleware from the rest of the http module
        //
        // the fallback here is a bit weird, we need to be careful that it redirects properly with
        // the app router's own fallback
        //
        // TODO -- generalize the auth middleware correctly
        let router = Router::new()
            .nest(&format!("{app_url_root}/app"), app_router)
            .nest(&format!("{app_url_root}/media"), media_router)
            .nest(&format!("{app_url_root}/api"), api_router)
            .fallback(move || async move { Redirect::permanent(&format!("{app_url_root}/app")) })
            .layer(TraceLayer::new_for_http())
            .route_layer(middleware::from_fn_with_state(
                config.authn_proxy_header.clone().unwrap(),
                proxy_auth,
            ));

        // everything from here follows the normal hyper/axum on tokio setup
        let service = hyper::service::service_fn(move |request: Request<hyper::body::Incoming>| {
            router.clone().call(request)
        });

        // for the moment, we just panic if the socket is in use
        let listener = tokio::net::TcpListener::bind(socket)
            .await
            .expect("http_service failed to bind tcp socket");

        // the main http server loop
        //
        // we want to return the handle to the caller, not the future, and so we just spawn it here
        let handle = tokio::task::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let service = service.clone();

                let io = hyper_util::rt::TokioIo::new(stream);

                tokio::task::spawn(async move {
                    match hyper_util::server::conn::auto::Builder::new(
                        hyper_util::rt::TokioExecutor::new(),
                    )
                    .serve_connection(io, service.clone())
                    .await
                    {
                        Ok(()) => (),
                        Err(err) => error!({service = "http_service", conn = "http", error = %err}),
                    }
                });
            }

            Err(anyhow::Error::msg("http_service http channel disconnected"))
        });

        debug!("started axum/hyper http listener");
        handle
    }
}

impl AuthCheck for HttpEndpoint {}

// http handlers

// note that all of these handlers return an AppError, which is a transparent
// wrapper around anyhow::Error so that we can impl IntoResponse
//
// this lets us use ? and ultimately send any errors all the way back to the
// caller, which simplifies everything drastically

// we use an axum Extension extractor to grab the CurrentUser struct inserted
// by the middleware auth layer; see http/auth.rs

// this relatively straightforward reader handles all types of media (images,
// thumbnails, video, etc), which will likely not work once we need video slices
async fn stream_media(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Path((dir, media_uuid)): Path<(String, i64)>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    if !state.can_access_media(&uid, &media_uuid).await? {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let media_uuid = media_uuid.to_string();

    // the pathbuf join() method inexplicably replaces the path if the argument
    // is absolute, so we include this explicit check
    if PathBuf::from(&dir).is_absolute() || PathBuf::from(&media_uuid).is_absolute() {
        return Ok(StatusCode::BAD_REQUEST.into_response());
    }

    // we only ever serve out of the linking directory, since we control its organization
    let filename = state.config.media_srvdir.join(dir).join(media_uuid);

    let file_handle = match tokio::fs::File::open(filename).await {
        Ok(f) => f,
        Err(err) => return Ok((StatusCode::NOT_FOUND, err.to_string()).into_response()),
    };

    let reader_stream = ReaderStream::new(file_handle);

    Ok(axum::body::Body::from_stream(reader_stream).into_response())
}

// auth handlers
async fn get_users_in_group(
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
async fn get_media(
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

async fn update_media(
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

async fn search_media(
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
                gid: gid,
                filter: message.filter,
            }
            .into(),
        )
        .await?;

    let result = rx.await??;

    Ok(Json(SearchMediaResp { media: result }).into_response())
}

async fn similar_media(
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
                gid: gid,
                media_uuid: message.media_uuid,
                distance: message.distance,
            }
            .into(),
        )
        .await?;

    let result = rx.await??;

    Ok(Json(SimilarMediaResp { media: result }).into_response())
}

async fn add_comment(
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
                    mtime: Local::now().timestamp(),
                    uid: uid,
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

async fn get_comment(
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

    let comment = rx
        .await??
        .ok_or_else(|| anyhow::Error::msg("unknown comment_uuid"))?;

    Ok(Json(GetCommentResp { comment: comment }).into_response())
}

async fn delete_comment(
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

async fn update_comment(
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

async fn add_collection(
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
                    uid: uid,
                    gid: message.collection.gid,
                    mtime: Local::now().timestamp(),
                    name: message.collection.name,
                    note: message.collection.note,
                    tags: message.collection.tags,
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

async fn get_collection(
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

async fn delete_collection(
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

async fn update_collection(
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

async fn add_media_to_collection(
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

async fn rm_media_from_collection(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<RmMediaFromCollectionReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    if !(state.owns_media(&uid, &message.media_uuid).await?
        && state
            .can_access_collection(&uid, &message.collection_uuid)
            .await?)
        && !state
            .owns_collection(&uid, &message.collection_uuid)
            .await?
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

async fn search_collections(
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
                gid: gid,
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

async fn search_media_in_collection(
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
                gid: gid,
                collection_uuid: message.collection_uuid,
                filter: message.filter,
            }
            .into(),
        )
        .await?;

    let result = rx.await??;

    Ok(Json(SearchMediaInCollectionResp { media: result }).into_response())
}

async fn get_library(
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

async fn search_libraries(
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
                gid: gid,
                filter: message.filter,
            }
            .into(),
        )
        .await?;

    let result = rx.await??;

    Ok(Json(SearchLibrariesResp { libraries: result }).into_response())
}

async fn search_media_in_library(
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
                gid: gid,
                library_uuid: message.library_uuid,
                filter: message.filter,
                hidden: message.hidden,
            }
            .into(),
        )
        .await?;

    let result = rx.await??;

    Ok(Json(SearchMediaInLibraryResp { media: result }).into_response())
}

async fn start_task(
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
                library_uuid: message.library_uuid,
                task_type: message.task_type,
                uid: TaskUid::User { uid },
            }
            .into(),
        )
        .await?;

    rx.await??;

    Ok(Json(StartTaskResp {}).into_response())
}

async fn stop_task(
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
                library_uuid: message.library_uuid,
            }
            .into(),
        )
        .await?;

    rx.await??;

    Ok(Json(StopTaskResp {}).into_response())
}

async fn show_tasks(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<ShowTasksReq>,
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
            TaskMsg::ShowTasks {
                resp: tx,
                library_uuid: message.library_uuid,
            }
            .into(),
        )
        .await?;

    let result = rx.await??;

    Ok(Json(ShowTasksResp { tasks: result }).into_response())
}
