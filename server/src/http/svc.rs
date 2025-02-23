use std::collections::{HashMap, HashSet};
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
use tracing::{debug, error, info, instrument, Level};

use crate::auth::msg::AuthMsg;
use crate::db::msg::DbMsg;
use crate::fs::msg::FsMsg;
use crate::http::{
    auth::{proxy_auth, CurrentUser},
    AppError,
};
use crate::service::{ESInner, ESMReceiver, ESMSender, EntanglementService, ServiceType, ESM};
use api::{album::*, comment::*, library::*, media::*};
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
// with none of the policy logic attached
pub struct HttpService {
    config: Arc<ESConfig>,
    receiver: Arc<Mutex<ESMReceiver>>,
    msg_handle: AsyncCell<tokio::task::JoinHandle<anyhow::Result<()>>>,
    hyper_handle: AsyncCell<tokio::task::JoinHandle<anyhow::Result<()>>>,
}

#[async_trait]
impl EntanglementService for HttpService {
    type Inner = HttpEndpoint;

    fn create(config: Arc<ESConfig>, sender_map: &mut HashMap<ServiceType, ESMSender>) -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel::<ESM>(1024);

        sender_map.insert(ServiceType::Http, tx);

        HttpService {
            config: config.clone(),
            receiver: Arc::new(Mutex::new(rx)),
            msg_handle: AsyncCell::new(),
            hyper_handle: AsyncCell::new(),
        }
    }

    #[instrument(level=Level::DEBUG, skip(self, senders))]
    async fn start(&self, senders: &HashMap<ServiceType, ESMSender>) -> anyhow::Result<()> {
        let receiver = Arc::clone(&self.receiver);
        let state = Arc::new(HttpEndpoint::new(self.config.clone(), senders.clone())?);

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

        debug!("finished startup for http_service");
        Ok(())
    }
}

// http endpoint
//
// this struct is the actual responder that binds to the socket and serves http
#[derive(Clone, Debug)]
pub struct HttpEndpoint {
    config: Arc<ESConfig>,
    auth_svc_sender: ESMSender,
    db_svc_sender: ESMSender,
    fs_svc_sender: ESMSender,
}

#[async_trait]
impl ESInner for HttpEndpoint {
    fn new(
        config: Arc<ESConfig>,
        senders: HashMap<ServiceType, ESMSender>,
    ) -> anyhow::Result<Self> {
        Ok(HttpEndpoint {
            config: config.clone(),
            // panic if we can't find all of the necessary senders, since this is a
            // compile-time problem and not a runtime problem
            auth_svc_sender: senders.get(&ServiceType::Auth).unwrap().clone(),
            db_svc_sender: senders.get(&ServiceType::Db).unwrap().clone(),
            fs_svc_sender: senders.get(&ServiceType::Fs).unwrap().clone(),
        })
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
    #[instrument(level=Level::DEBUG, skip_all)]
    async fn serve_http(
        self: Arc<Self>,
        socket: SocketAddr,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
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
            .route("/GetMedia", post(get_media))
            .route("/UpdateMedia", post(update_media))
            .route("/SearchMedia", post(search_media))
            .route("/AddComment", post(add_comment))
            .route("/GetComment", post(get_comment))
            .route("/DeleteComment", post(delete_comment))
            .route("/UpdateComment", post(update_comment))
            .route("/AddAlbum", post(add_album))
            .route("/GetAlbum", post(get_album))
            .route("/DeleteAlbum", post(delete_album))
            .route("/UpdateAlbum", post(update_album))
            .route("/AddMediaToAlbum", post(add_media_to_album))
            .route("/RmMediaFromAlbum", post(rm_media_from_album))
            .route("/SearchAlbums", post(search_albums))
            .route("/SearchMediaInAlbum", post(search_media_in_album))
            .route("/GetLibrary", post(get_library))
            .route("/SearchLibraries", post(search_libraries))
            .route("/SearchMediaInLibrary", post(search_media_in_library))
            .route("/StartLibraryScan", post(start_library_scan))
            .route("/GetLibraryScan", post(get_library_scan))
            .route("/StopLibraryScan", post(stop_library_scan))
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
        let listener = tokio::net::TcpListener::bind(socket).await.expect("http_service failed to bind tcp socket");

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

        debug!("finished startup for serve_http");
        handle
    }

    #[instrument(level=Level::DEBUG)]
    async fn groups_for_user(&self, uid: &String) -> anyhow::Result<HashSet<String>> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.auth_svc_sender
            .send(
                AuthMsg::GroupsForUser {
                    resp: tx,
                    uid: uid.clone(),
                }
                .into(),
            )
            .await?;

        rx.await?
    }

    #[instrument(level=Level::DEBUG)]
    async fn is_group_member(&self, uid: &String, gid: HashSet<String>) -> anyhow::Result<bool> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.auth_svc_sender
            .send(
                AuthMsg::IsGroupMember {
                    resp: tx,
                    uid: uid.clone(),
                    gid: gid,
                }
                .into(),
            )
            .await?;

        rx.await?
    }

    #[instrument(level=Level::DEBUG)]
    async fn can_access_media(&self, uid: &String, media_uuid: &MediaUuid) -> anyhow::Result<bool> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.auth_svc_sender
            .send(
                AuthMsg::CanAccessMedia {
                    resp: tx,
                    uid: uid.clone(),
                    media_uuid: media_uuid.clone(),
                }
                .into(),
            )
            .await?;

        rx.await?
    }

    #[instrument(level=Level::DEBUG)]
    async fn owns_media(&self, uid: &String, media_uuid: &MediaUuid) -> anyhow::Result<bool> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.auth_svc_sender
            .send(
                AuthMsg::OwnsMedia {
                    resp: tx,
                    uid: uid.clone(),
                    media_uuid: media_uuid.clone(),
                }
                .into(),
            )
            .await?;

        rx.await?
    }

    #[instrument(level=Level::DEBUG)]
    async fn can_access_comment(
        &self,
        uid: &String,
        comment_uuid: &CommentUuid,
    ) -> anyhow::Result<bool> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.db_svc_sender
            .send(
                DbMsg::GetComment {
                    resp: tx,
                    comment_uuid: comment_uuid.clone(),
                }
                .into(),
            )
            .await?;

        let comment = rx
            .await??
            .ok_or_else(|| anyhow::Error::msg("unknown comment_uuid"))?;

        self.can_access_media(uid, &comment.media_uuid).await
    }

    #[instrument(level=Level::DEBUG)]
    async fn owns_comment(&self, uid: &String, comment_uuid: &CommentUuid) -> anyhow::Result<bool> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.db_svc_sender
            .send(
                DbMsg::GetComment {
                    resp: tx,
                    comment_uuid: comment_uuid.clone(),
                }
                .into(),
            )
            .await?;

        let comment = rx
            .await??
            .ok_or_else(|| anyhow::Error::msg("unknown comment_uuid"))?;

        Ok(uid.to_owned() == comment.uid)
    }

    #[instrument(level=Level::DEBUG)]
    async fn can_access_album(&self, uid: &String, album_uuid: &AlbumUuid) -> anyhow::Result<bool> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.db_svc_sender
            .send(
                DbMsg::GetAlbum {
                    resp: tx,
                    album_uuid: album_uuid.clone(),
                }
                .into(),
            )
            .await?;

        let album = rx
            .await??
            .ok_or_else(|| anyhow::Error::msg("unknown album_uuid"))?;

        self.is_group_member(&uid, HashSet::from([album.gid])).await
    }

    #[instrument(level=Level::DEBUG)]
    async fn owns_album(&self, uid: &String, album_uuid: &AlbumUuid) -> anyhow::Result<bool> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.db_svc_sender
            .send(
                DbMsg::GetAlbum {
                    resp: tx,
                    album_uuid: album_uuid.clone(),
                }
                .into(),
            )
            .await?;

        let album = rx
            .await??
            .ok_or_else(|| anyhow::Error::msg("unknown album_uuid"))?;

        Ok(uid.to_owned() == album.uid)
    }

    #[instrument(level=Level::DEBUG)]
    async fn owns_library(&self, uid: &String, library_uuid: &LibraryUuid) -> anyhow::Result<bool> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.db_svc_sender
            .send(
                DbMsg::GetLibrary {
                    resp: tx,
                    library_uuid: library_uuid.clone(),
                }
                .into(),
            )
            .await?;

        let library = rx
            .await??
            .ok_or_else(|| anyhow::Error::msg("unknown library_uuid"))?;

        self.is_group_member(&uid, HashSet::from([library.gid]))
            .await
    }
}

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
        albums: result.1,
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
                uid: uid,
                gid: gid,
                filter: message.filter,
            }
            .into(),
        )
        .await?;

    let result = rx.await??;

    Ok(Json(SearchMediaResp { media: result }).into_response())
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

async fn add_album(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<AddAlbumReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    // anyone may create an album, but they must be in the group they are creating
    if !state
        .is_group_member(&uid, HashSet::from([message.gid.clone()]))
        .await?
    {
        return Err(anyhow::Error::msg("User must be a member of album group").into());
    }

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::AddAlbum {
                resp: tx,
                album: Album {
                    uid: uid,
                    gid: message.gid,
                    mtime: Local::now().timestamp(),
                    name: message.name,
                    note: message.note,
                },
            }
            .into(),
        )
        .await?;

    let result = rx.await??;

    Ok(Json(AddAlbumResp { album_uuid: result }).into_response())
}

async fn get_album(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<GetAlbumReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    if !state.can_access_album(&uid, &message.album_uuid).await? {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::GetAlbum {
                resp: tx,
                album_uuid: message.album_uuid,
            }
            .into(),
        )
        .await?;

    let result = rx
        .await??
        .ok_or_else(|| anyhow::Error::msg("unknown album_uuid"))?;

    Ok(Json(GetAlbumResp { album: result }).into_response())
}

async fn delete_album(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<DeleteAlbumReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    if !state.owns_album(&uid, &message.album_uuid).await? {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::DeleteAlbum {
                resp: tx,
                album_uuid: message.album_uuid,
            }
            .into(),
        )
        .await?;

    rx.await??;

    Ok(Json(DeleteAlbumResp {}).into_response())
}

async fn update_album(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<UpdateAlbumReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    if !state.owns_album(&uid, &message.album_uuid).await? {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::UpdateAlbum {
                resp: tx,
                album_uuid: message.album_uuid,
                update: message.update,
            }
            .into(),
        )
        .await?;

    rx.await??;

    Ok(Json(UpdateAlbumResp {}).into_response())
}

async fn add_media_to_album(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<AddMediaToAlbumReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    if !state.owns_media(&uid, &message.media_uuid).await? {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    if !state.can_access_album(&uid, &message.album_uuid).await? {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::AddMediaToAlbum {
                resp: tx,
                media_uuid: message.media_uuid,
                album_uuid: message.album_uuid,
            }
            .into(),
        )
        .await?;

    rx.await??;

    Ok(Json(AddMediaToAlbumResp {}).into_response())
}

async fn rm_media_from_album(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<RmMediaFromAlbumReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    if !(state.owns_media(&uid, &message.media_uuid).await?
        && state.can_access_album(&uid, &message.album_uuid).await?)
        && !state.owns_album(&uid, &message.album_uuid).await?
    {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::RmMediaFromAlbum {
                resp: tx,
                media_uuid: message.media_uuid,
                album_uuid: message.album_uuid,
            }
            .into(),
        )
        .await?;

    rx.await??;

    Ok(Json(RmMediaFromAlbumResp {}).into_response())
}

async fn search_albums(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<SearchAlbumsReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    // auth handled in db search

    let gid = state.groups_for_user(&uid).await?;

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::SearchAlbums {
                resp: tx,
                uid: uid,
                gid: gid,
                filter: message.filter,
            }
            .into(),
        )
        .await?;

    let result = rx.await??;

    Ok(Json(SearchAlbumsResp { albums: result }).into_response())
}

async fn search_media_in_album(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<SearchMediaInAlbumReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    // auth handled in db search

    let gid = state.groups_for_user(&uid).await?;

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .send(
            DbMsg::SearchMediaInAlbum {
                resp: tx,
                uid: uid,
                gid: gid,
                album_uuid: message.album_uuid,
                filter: message.filter,
            }
            .into(),
        )
        .await?;

    let result = rx.await??;

    Ok(Json(SearchMediaInAlbumResp { media: result }).into_response())
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
                uid: uid,
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
                uid: uid,
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

async fn start_library_scan(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<StartLibraryScanReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    if !state.owns_library(&uid, &message.library_uuid).await? {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .fs_svc_sender
        .send(
            FsMsg::ScanLibrary {
                resp: tx,
                library_uuid: message.library_uuid,
            }
            .into(),
        )
        .await?;

    rx.await??;

    Ok(Json(StartLibraryScanResp {}).into_response())
}

async fn get_library_scan(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(_message): Json<GetLibraryScanReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    if !state
        .is_group_member(&uid, state.config.authz_admin_groups.clone())
        .await?
    {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .fs_svc_sender
        .send(FsMsg::ScanStatus { resp: tx }.into())
        .await?;

    let jobs = rx.await??;

    Ok(Json(GetLibraryScanResp { jobs }).into_response())
}

async fn stop_library_scan(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<StopLibraryScanReq>,
) -> Result<Response, AppError> {
    let state = state.clone();
    let uid = current_user.uid.clone();

    if !state
        .is_group_member(&uid, state.config.authz_admin_groups.clone())
        .await?
    {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .fs_svc_sender
        .send(
            FsMsg::StopScan {
                resp: tx,
                library_uuid: message.library_uuid,
            }
            .into(),
        )
        .await?;

    rx.await??;

    Ok(Json(StopLibraryScanResp {}).into_response())
}
