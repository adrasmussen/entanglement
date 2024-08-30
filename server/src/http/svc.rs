use std::collections::{HashMap, HashSet};
use std::net::{SocketAddr, SocketAddrV6};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{self, Context};

use async_cell::sync::AsyncCell;

use async_trait::async_trait;

use axum::{
    extract::{Extension, Json, Path, Request, State},
    http::{StatusCode, Uri},
    middleware,
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Router,
};

use chrono::offset::Local;

use tokio::sync::Mutex;

use tokio_util::io::ReaderStream;

use tower::Service;

use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

use crate::auth::{get_admin_groups, msg::AuthMsg};
use crate::db::msg::DbMsg;
use crate::fs::msg::FsMsg;
use crate::http::{
    auth::{proxy_auth, CurrentUser},
    AppError,
};
use crate::service::*;
use api::{album::*, group::*, library::*, media::*, ticket::*, user::*};

#[derive(Clone, Debug)]
pub struct HttpEndpoint {
    auth_svc_sender: ESMSender,
    db_svc_sender: ESMSender,
    fs_svc_sender: ESMSender,
    media_linkdir: PathBuf,
}

impl HttpEndpoint {
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
            .await
            .context("Failed to send IsGroupMember message in is_group_member")?;

        rx.await
            .context("Failed to receive IsGroupMember response at is_group_member")?
    }

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
            .await
            .context("Failed to send CanAccessMedia message in can_access_media")?;

        rx.await
            .context("Failed to receive CanAccessMedia response at can_access_media")?
    }

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
            .await
            .context("Failed to send CanAccessMedia message from owns_media")?;

        rx.await
            .context("Failed to receive CanAccessMedia response at owns_media")?
    }

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
            .await
            .context("Failed to send GetAlbum message from can_access_album")?;

        let album = rx
            .await
            .context("Failed to receive GetAlbum response at can_access_album")??
            .ok_or_else(|| anyhow::Error::msg("unknown album_uuid"))?;

        self.is_group_member(&uid, HashSet::from([album.gid])).await
    }

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
            .await
            .context("Failed to send GetAlbum message from owns_album")?;

        let album = rx
            .await
            .context("Failed to receive GetAlbum response at owns_album")??
            .ok_or_else(|| anyhow::Error::msg("unknown album_uuid"))?;

        Ok(uid.to_owned() == album.uid)
    }

    async fn can_access_library(
        &self,
        uid: &String,
        library_uuid: &LibraryUuid,
    ) -> anyhow::Result<bool> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.db_svc_sender
            .send(
                DbMsg::GetLibrary {
                    resp: tx,
                    library_uuid: library_uuid.clone(),
                }
                .into(),
            )
            .await
            .context("Failed to send GetLibrary message from can_access_library")?;

        let library = rx
            .await
            .context("Failed to receive GetLibrary response at can_access_library")??
            .ok_or_else(|| anyhow::Error::msg("unknown library_uuid"))?;

        self.is_group_member(&uid, HashSet::from([library.gid]))
            .await
    }

    async fn can_access_ticket(
        &self,
        uid: &String,
        ticket_uuid: &TicketUuid,
    ) -> anyhow::Result<bool> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.db_svc_sender
            .send(
                DbMsg::GetTicket {
                    resp: tx,
                    ticket_uuid: ticket_uuid.clone(),
                }
                .into(),
            )
            .await
            .context("Failed to send GetTicket from HttpEndpoint")?;

        let ticket = rx
            .await
            .context("Failed to receive GetTicket response at HttpEndpoint")??
            .ok_or_else(|| anyhow::Error::msg("unknown ticket_uuid"))?;

        self.can_access_media(&uid, &ticket.media_uuid).await
    }

    async fn owns_ticket(&self, uid: &String, ticket_uuid: &TicketUuid) -> anyhow::Result<bool> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.db_svc_sender
            .send(
                DbMsg::GetTicket {
                    resp: tx,
                    ticket_uuid: ticket_uuid.clone(),
                }
                .into(),
            )
            .await
            .context("Failed to send GetTicket from HttpEndpoint")?;

        let ticket = rx
            .await
            .context("Failed to receive GetTicket response at HttpEndpoint")??
            .ok_or_else(|| anyhow::Error::msg("unknown ticket_uuid"))?;

        Ok(uid.to_owned() == ticket.uid)
    }
}

#[async_trait]
impl ESInner for HttpEndpoint {
    fn new(
        config: Arc<ESConfig>,
        senders: HashMap<ServiceType, ESMSender>,
    ) -> anyhow::Result<Self> {
        Ok(HttpEndpoint {
            // panic if we can't find all of the necessary senders, since this is a
            // compile-time problem and not a runtime problem
            auth_svc_sender: senders.get(&ServiceType::Auth).unwrap().clone(),
            db_svc_sender: senders.get(&ServiceType::Db).unwrap().clone(),
            fs_svc_sender: senders.get(&ServiceType::Fs).unwrap().clone(),
            media_linkdir: config.media_linkdir.clone(),
        })
    }

    async fn message_handler(&self, esm: ESM) -> anyhow::Result<()> {
        match esm {
            _ => Err(anyhow::Error::msg("not implemented")),
        }
    }
}

pub struct HttpService {
    config: Arc<ESConfig>,
    receiver: Arc<Mutex<ESMReceiver>>,
    msg_handle: AsyncCell<tokio::task::JoinHandle<anyhow::Result<()>>>,
    hyper_handle: AsyncCell<tokio::task::JoinHandle<anyhow::Result<()>>>,
}

#[async_trait]
impl EntanglementService for HttpService {
    type Inner = HttpEndpoint;

    fn create(config: Arc<ESConfig>) -> (ESMSender, Self) {
        let (tx, rx) = tokio::sync::mpsc::channel::<ESM>(32);

        (
            tx,
            HttpService {
                config: config.clone(),
                receiver: Arc::new(Mutex::new(rx)),
                msg_handle: AsyncCell::new(),
                hyper_handle: AsyncCell::new(),
            },
        )
    }

    async fn start(&self, senders: HashMap<ServiceType, ESMSender>) -> anyhow::Result<()> {
        let receiver = Arc::clone(&self.receiver);
        let state = Arc::new(HttpEndpoint::new(self.config.clone(), senders)?);

        // this will eventually come from the config
        let socket = SocketAddr::from(
            self.config
                .http_socket
                .parse::<SocketAddrV6>()
                .expect("Failed to parse http_socket ipv6 address/port"),
        );

        let hyper_handle = tokio::task::spawn(serve_http(socket, Arc::clone(&state)));

        self.hyper_handle.set(hyper_handle);

        let msg_serve = {
            async move {
                while let Some(msg) = receiver.lock().await.recv().await {
                    let state = Arc::clone(&state);
                    tokio::task::spawn(async move {
                        match state.message_handler(msg).await {
                            Ok(()) => (),
                            Err(_) => println!("http service failed to reply to message"),
                        }
                    });
                }

                Err::<(), anyhow::Error>(anyhow::Error::msg(format!("channel disconnected")))
            }
        };

        let msg_handle = tokio::task::spawn(msg_serve);

        self.msg_handle.set(msg_handle);

        Ok(())
    }
}

async fn serve_http(socket: SocketAddr, state: Arc<HttpEndpoint>) -> Result<(), anyhow::Error> {
    let state = Arc::clone(&state);

    let app_url_root = "/entanglement";
    let app_web_dir = "/tmp/entanglement/dist";

    let app_router = Router::new().nest_service(
        "/",
        ServeDir::new(app_web_dir).fallback(ServeFile::new(
            PathBuf::from(app_web_dir).join("index.html"),
        )),
    );

    let api_router: Router<()> = Router::new()
        .route("/user", post(query_user))
        .route("/group", post(query_group))
        .route("/media", post(query_media))
        .route("/album", post(query_album))
        .route("/library", post(query_library))
        .route("/ticket", post(query_ticket))
        .with_state(state.clone());

    let media_router = Router::new()
        .route("/:dir/:media_uuid", get(stream_media))
        .with_state(state);

    let router = Router::new()
        .nest(&format!("{app_url_root}/app"), app_router)
        .nest(&format!("{app_url_root}/api"), api_router)
        .nest(&format!("{app_url_root}/media"), media_router)
        .fallback(move || async move { Redirect::permanent(&format!("{app_url_root}/app")) })
        .layer(TraceLayer::new_for_http())
        .route_layer(middleware::from_fn(proxy_auth));

    let service = hyper::service::service_fn(move |request: Request<hyper::body::Incoming>| {
        router.clone().call(request)
    });

    // for the moment, we just fail if the socket is in use
    let listener = tokio::net::TcpListener::bind(socket).await.unwrap();

    // the main http server loop
    while let Ok((stream, _)) = listener.accept().await {
        let service = service.clone();

        let io = hyper_util::rt::TokioIo::new(stream);

        tokio::task::spawn(async move {
            match hyper_util::server::conn::auto::Builder::new(hyper_util::rt::TokioExecutor::new())
                .serve_connection(io, service.clone())
                .await
            {
                Ok(()) => (),
                Err(err) => println!("{}", err.to_string()),
            }
        });
    }

    Ok(())
}

async fn stream_media(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Path((dir, media_uuid)): Path<(String, i64)>,
) -> Result<Response, AppError> {
    let state = state.clone();

    let uid = current_user.uid.clone();

    // auth
    if !state.can_access_media(&uid, &media_uuid).await? {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    // once we've passed the auth check, we get the file handle and build
    // a streaming body from it
    let filename = state.media_linkdir.join(dir).join(media_uuid.to_string());

    let file_handle = match tokio::fs::File::open(filename).await {
        Ok(f) => f,
        Err(err) => return Ok((StatusCode::NOT_FOUND, err.to_string()).into_response()),
    };

    let reader_stream = ReaderStream::new(file_handle);

    Ok(axum::body::Body::from_stream(reader_stream).into_response())
}

// need to set up a query here to handle each use case

async fn query_user(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<UserMessage>,
) -> Result<Response, AppError> {
    let state = state.clone();

    let uid = current_user.uid.clone();

    match message {
        UserMessage::CreateUser(msg) => {
            // auth
            //
            // admins can create users
            if !state.is_group_member(&uid, get_admin_groups()).await? {
                return Ok(StatusCode::UNAUTHORIZED.into_response());
            }

            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .send(
                    DbMsg::CreateUser {
                        resp: tx,
                        uid: msg.uid,
                        metadata: msg.metadata,
                    }
                    .into(),
                )
                .await
                .context("Failed to send CreateUser message")?;

            rx.await
                .context("Failed to receive CreateUser response")??;

            Ok(Json(CreateUserResp {}).into_response())
        }
        UserMessage::GetUser(msg) => {
            // auth
            //
            // admins and self can inspect users
            if !(uid == msg.uid) && !state.is_group_member(&uid, get_admin_groups()).await? {
                return Ok(StatusCode::UNAUTHORIZED.into_response());
            }

            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .send(
                    DbMsg::GetUser {
                        resp: tx,
                        uid: msg.uid,
                    }
                    .into(),
                )
                .await
                .context("Failed to send GetUser message")?;

            let result = rx
                .await
                .context("Failed to receive GetUser response")??
                .ok_or_else(|| anyhow::Error::msg("unknown uid"))?;

            Ok(Json(GetUserResp { user: result }).into_response())
        }
        UserMessage::DeleteUser(msg) => {
            // auth
            //
            // admins may delete users
            if !state.is_group_member(&uid, get_admin_groups()).await? {
                return Ok(StatusCode::UNAUTHORIZED.into_response());
            }

            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .send(
                    DbMsg::DeleteUser {
                        resp: tx,
                        uid: msg.uid,
                    }
                    .into(),
                )
                .await
                .context("Failed to send DeleteUser message")?;

            rx.await
                .context("Failed to receive DeleteUser response")??;

            Ok(Json(DeleteUserResp {}).into_response())
        }
    }
}

async fn query_group(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<GroupMessage>,
) -> Result<Response, AppError> {
    let state = state.clone();

    let uid = current_user.uid.clone();

    match message {
        GroupMessage::CreateGroup(msg) => {
            // auth
            //
            // admins can create groups
            if !state.is_group_member(&uid, get_admin_groups()).await? {
                return Ok(StatusCode::UNAUTHORIZED.into_response());
            }

            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .send(
                    DbMsg::CreateGroup {
                        resp: tx,
                        gid: msg.gid,
                        metadata: msg.metadata,
                    }
                    .into(),
                )
                .await
                .context("Failed to send CreateGroup message")?;

            rx.await
                .context("Failed to receive CreateGroup response")??;

            Ok(Json(CreateGroupResp {}).into_response())
        }
        GroupMessage::GetGroup(msg) => {
            // auth
            //
            // members of a group may fetch it
            if !state
                .is_group_member(&uid, HashSet::from([msg.gid.clone()]))
                .await?
            {
                return Ok(StatusCode::UNAUTHORIZED.into_response());
            }

            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .send(
                    DbMsg::GetGroup {
                        resp: tx,
                        gid: msg.gid,
                    }
                    .into(),
                )
                .await
                .context("Failed to send GetGroup message")?;

            let result = rx
                .await
                .context("Failed to receive GetGroup response")??
                .ok_or_else(|| anyhow::Error::msg("unknown gid"))?;

            Ok(Json(GetGroupResp { group: result }).into_response())
        }
        GroupMessage::DeleteGroup(msg) => {
            // auth
            //
            // admins can delete groups
            if !state.is_group_member(&uid, get_admin_groups()).await? {
                return Ok(StatusCode::UNAUTHORIZED.into_response());
            }

            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .send(
                    DbMsg::DeleteGroup {
                        resp: tx,
                        gid: msg.gid,
                    }
                    .into(),
                )
                .await
                .context("Failed to send DeleteGroup message")?;

            rx.await
                .context("Failed to receive DeleteGroup response")??;

            Ok(Json(DeleteGroupResp {}).into_response())
        }
        GroupMessage::AddUserToGroup(msg) => {
            // auth
            //
            // admins can add users to groups
            if !state.is_group_member(&uid, get_admin_groups()).await? {
                return Ok(StatusCode::UNAUTHORIZED.into_response());
            }

            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .send(
                    DbMsg::AddUserToGroup {
                        resp: tx,
                        uid: msg.uid,
                        gid: msg.gid,
                    }
                    .into(),
                )
                .await
                .context("Failed to send AddUserToGroup message")?;

            rx.await
                .context("Failed to receive AddUserToGroup resposne")??;

            Ok(Json(AddUserToGroupResp {}).into_response())
        }
        GroupMessage::RmUserFromGroup(msg) => {
            // auth
            //
            // admins can remove users from groups
            if !state.is_group_member(&uid, get_admin_groups()).await? {
                return Ok(StatusCode::UNAUTHORIZED.into_response());
            }

            // sanity check
            if (&msg.uid == &uid) && get_admin_groups().contains(&msg.gid) {
                return Err(anyhow::Error::msg("cannot remove self from admin group").into());
            }

            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .send(
                    DbMsg::RmUserFromGroup {
                        resp: tx,
                        uid: msg.uid,
                        gid: msg.gid,
                    }
                    .into(),
                )
                .await
                .context("Failed to send RmUserFromGroup message")?;

            rx.await
                .context("Failed to receive RmUserFromGroup response")??;

            Ok(Json(RmUserFromGroupResp {}).into_response())
        }
    }
}

async fn query_media(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<MediaMessage>,
) -> Result<Response, AppError> {
    let state = state.clone();

    let uid = current_user.uid.clone();

    match message {
        MediaMessage::GetMedia(msg) => {
            // auth
            if !state.can_access_media(&uid, &msg.media_uuid).await? {
                return Ok(StatusCode::UNAUTHORIZED.into_response());
            }

            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .send(
                    DbMsg::GetMedia {
                        resp: tx,
                        media_uuid: msg.media_uuid,
                    }
                    .into(),
                )
                .await
                .context("Failed to send GetMedia message")?;

            let result = rx
                .await
                .context("Failed to receive GetMedia response")??
                .ok_or_else(|| anyhow::Error::msg("unknown media_uuid"))?;

            Ok(Json(GetMediaResp { media: result }).into_response())
        }
        MediaMessage::UpdateMedia(msg) => {
            // auth
            if !state.owns_media(&uid, &msg.media_uuid).await? {
                return Ok(StatusCode::UNAUTHORIZED.into_response());
            }

            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .send(
                    DbMsg::UpdateMedia {
                        resp: tx,
                        media_uuid: msg.media_uuid,
                        change: msg.change,
                    }
                    .into(),
                )
                .await
                .context("Failed to send UpdateMedia message")?;

            rx.await
                .context("Failed to receive UpdateMedia response")??;

            Ok(Json(UpdateMediaResp {}).into_response())
        }
        MediaMessage::SetMediaHidden(msg) => {
            // auth
            if !state.owns_media(&uid, &msg.media_uuid).await? {
                return Ok(StatusCode::UNAUTHORIZED.into_response());
            }

            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .send(
                    DbMsg::SetMediaHidden {
                        resp: tx,
                        media_uuid: msg.media_uuid,
                        hidden: msg.hidden,
                    }
                    .into(),
                )
                .await
                .context("Failed to send UpdateMedia message")?;

            rx.await
                .context("Failed to receive UpdateMedia response")??;

            Ok(Json(SetMediaHiddenResp {}).into_response())
        }
        MediaMessage::SearchMedia(msg) => {
            // auth
            //
            // handled as part of search query
            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .send(
                    DbMsg::SearchMedia {
                        resp: tx,
                        uid: uid,
                        filter: msg.filter,
                    }
                    .into(),
                )
                .await
                .context("Failed to send SearchMedia message")?;

            let result = rx
                .await
                .context("Failed to receive SearchMedia message")??;

            Ok(Json(SearchMediaResp { media: result }).into_response())
        }
        MediaMessage::RevSearchMediaForAlbum(msg) => {
            Err(anyhow::Error::msg("not implemented").into())
        }
    }
}

async fn query_album(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<AlbumMessage>,
) -> Result<Response, AppError> {
    let state = state.clone();

    let uid = current_user.uid.clone();

    match message {
        AlbumMessage::CreateAlbum(msg) => {
            // auth
            //
            // anyone may create an album, but they must be in the group they are creating
            if !state
                .is_group_member(&uid, HashSet::from([msg.album.gid.clone()]))
                .await?
            {
                return Err(anyhow::Error::msg("User must be a member of album group").into());
            }

            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .send(
                    DbMsg::CreateAlbum {
                        resp: tx,
                        album: msg.album,
                    }
                    .into(),
                )
                .await
                .context("Failed to send CreateAlbum message")?;

            let result = rx
                .await
                .context("Failed to receive CreateAlbum response")??;

            Ok(Json(CreateAlbumResp { album_uuid: result }).into_response())
        }
        AlbumMessage::GetAlbum(msg) => {
            // auth
            //
            // anyone in the album group may fetch an album
            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .send(
                    DbMsg::GetAlbum {
                        resp: tx,
                        album_uuid: msg.album_uuid,
                    }
                    .into(),
                )
                .await
                .context("Failed to send GetAlbum message")?;

            let result = rx
                .await
                .context("Failed to receive GetAlbum response")??
                .ok_or_else(|| anyhow::Error::msg("unknown album_uuid"))?;

            Ok(Json(GetAlbumResp { album: result }).into_response())
        }
        AlbumMessage::DeleteAlbum(msg) => {
            // auth
            //
            // the album owner (and admins) may delete an album
            if !state.owns_album(&uid, &msg.album_uuid).await?
                && !state.is_group_member(&uid, get_admin_groups()).await?
            {
                return Ok(StatusCode::UNAUTHORIZED.into_response());
            }

            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .send(
                    DbMsg::DeleteAlbum {
                        resp: tx,
                        album_uuid: msg.album_uuid,
                    }
                    .into(),
                )
                .await
                .context("Failed to send DeleteAlbum message")?;

            rx.await
                .context("Failed to receive DeleteAlbum response")??;

            Ok(Json(DeleteAlbumResp {}).into_response())
        }
        AlbumMessage::UpdateAlbum(msg) => {
            // auth
            //
            // the album owner may update album metadata
            if !state.owns_album(&uid, &msg.album_uuid).await? {
                return Ok(StatusCode::UNAUTHORIZED.into_response());
            }

            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .send(
                    DbMsg::UpdateAlbum {
                        resp: tx,
                        album_uuid: msg.album_uuid,
                        change: msg.metadata,
                    }
                    .into(),
                )
                .await
                .context("Failed to send UpdateAlbum message")?;

            rx.await
                .context("Failed to receive UpdateAlbum response")??;

            Ok(Json(UpdateAlbumResp {}).into_response())
        }
        AlbumMessage::AddMediaToAlbum(msg) => {
            // auth
            //
            // a user must own the media and be able to see an album
            // in order to add the first to the second
            if !state.owns_media(&uid, &msg.media_uuid).await?
                || !state.can_access_album(&uid, &msg.album_uuid).await?
            {
                return Ok(StatusCode::UNAUTHORIZED.into_response());
            }

            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .send(
                    DbMsg::AddMediaToAlbum {
                        resp: tx,
                        media_uuid: msg.media_uuid,
                        album_uuid: msg.album_uuid,
                    }
                    .into(),
                )
                .await
                .context("Failed to send AddMediaToAlbum message")?;

            rx.await
                .context("Failed to receive AddMediaToAlbum response")??;

            Ok(Json(AddMediaToAlbumResp {}).into_response())
        }
        AlbumMessage::RmMediaFromAlbum(msg) => {
            // auth
            //
            // a user can either own the media and see the album or
            // own the album to remove media
            if !state.owns_album(&uid, &msg.album_uuid).await?
                && (!state.owns_media(&uid, &msg.media_uuid).await?
                    || !state.can_access_album(&uid, &msg.album_uuid).await?)
            {
                return Ok(StatusCode::UNAUTHORIZED.into_response());
            }

            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .send(
                    DbMsg::RmMediaFromAlbum {
                        resp: tx,
                        media_uuid: msg.media_uuid,
                        album_uuid: msg.album_uuid,
                    }
                    .into(),
                )
                .await
                .context("Failed to send RmMediaFromAlbum message")?;

            rx.await
                .context("Failed to receive RmMediaFromAlbum response")??;

            Ok(Json(RmMediaFromAlbumResp {}).into_response())
        }
        AlbumMessage::SearchAlbums(msg) => {
            // auth
            //
            // handled as part of search query
            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .send(
                    DbMsg::SearchAlbums {
                        resp: tx,
                        user: uid,
                        filter: msg.filter,
                    }
                    .into(),
                )
                .await
                .context("Failed to send SearchAlbums message")?;

            let result = rx
                .await
                .context("Failed to receive SearchAlbums response")??;

            Ok(Json(SearchAlbumsResp { albums: result }).into_response())
        }
        AlbumMessage::SearchMediaInAlbum(msg) => {
            // auth
            //
            // handled as part of search query
            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .send(
                    DbMsg::SearchMediaInAlbum {
                        resp: tx,
                        user: uid,
                        album_uuid: msg.album_uuid,
                        filter: msg.filter,
                    }
                    .into(),
                )
                .await
                .context("Failed to send SearchMediaInAlbum message")?;

            let result = rx
                .await
                .context("Failed to receive SearchMediaInAlbum response")??;

            Ok(Json(SearchMediaInAlbumResp { media: result }).into_response())
        }
    }
}

async fn query_library(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<LibraryMessage>,
) -> Result<Response, AppError> {
    let state = state.clone();

    let uid = current_user.uid.clone();

    match message {
        LibraryMessage::AddLibrary(msg) => {
            if !state.is_group_member(&uid, get_admin_groups()).await? {
                return Ok(StatusCode::UNAUTHORIZED.into_response());
            }

            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .send(
                    DbMsg::AddLibrary {
                        resp: tx,
                        library: msg.library,
                    }
                    .into(),
                )
                .await
                .context("Failed to send AddLibrary message")?;

            let result = rx
                .await
                .context("Failed to receive AddLibrary response")??;

            Ok(Json(AddLibaryResp {
                library_uuid: result,
            })
            .into_response())
        }
        LibraryMessage::GetLibary(msg) => {
            // auth
            //
            // anyone in the library group can see the library
            if !state.can_access_library(&uid, &msg.library_uuid).await? {
                return Ok(StatusCode::UNAUTHORIZED.into_response());
            }

            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .send(
                    DbMsg::GetLibrary {
                        resp: tx,
                        library_uuid: msg.library_uuid,
                    }
                    .into(),
                )
                .await
                .context("Failed to send GetLibrary message")?;

            let result = rx
                .await
                .context("Failed to receive GetLibrary response")??
                .ok_or_else(|| anyhow::Error::msg("unknown library_uuid"))?;

            Ok(Json(GetLibaryResp { library: result }).into_response())
        }
        LibraryMessage::SearchMediaInLibrary(msg) => {
            // auth
            //
            // anyone in the library group can search in the library
            if !state.can_access_library(&uid, &msg.library_uuid).await? {
                return Ok(StatusCode::UNAUTHORIZED.into_response());
            }

            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .send(
                    DbMsg::SearchMediaInLibrary {
                        resp: tx,
                        uid: uid.clone(),
                        library_uuid: msg.library_uuid,
                        filter: msg.filter,
                        hidden: msg.hidden,
                    }
                    .into(),
                )
                .await
                .context("Failed to send SearchMediaInLibrary message")?;

            let result = rx
                .await
                .context("Failed to receive SearchMediaInLibrary response")??;

            Ok(Json(SearchMediaInLibraryResp { media: result }).into_response())
        }
        LibraryMessage::ScanLibrary(msg) => {
            // auth
            //
            // anyone who owns a library (and admins) may start a scan
            if !state.can_access_library(&uid, &msg.library_uuid).await?
                && !state.is_group_member(&uid, get_admin_groups()).await?
            {
                return Ok(StatusCode::UNAUTHORIZED.into_response());
            }

            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .fs_svc_sender
                .send(
                    FsMsg::ScanLibrary {
                        resp: tx,
                        library_uuid: msg.library_uuid,
                    }
                    .into(),
                )
                .await
                .context("Failed to send ScanLibrary message")?;

            let result = rx
                .await
                .context("Failed to receive ScanLibary response")??;

            Ok(Json(ScanLibraryResp { result: result }).into_response())
        }
    }
}

async fn query_ticket(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<TicketMessage>,
) -> Result<Response, AppError> {
    let state = state.clone();

    let uid = current_user.uid.clone();

    match message {
        TicketMessage::CreateTicket(msg) => {
            // auth
            //
            // anyone who can see media can create a new ticket, even if they don't own it
            if !state.can_access_media(&uid, &msg.media_uuid).await? {
                return Ok(StatusCode::UNAUTHORIZED.into_response());
            }

            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .send(
                    DbMsg::CreateTicket {
                        resp: tx,
                        ticket: Ticket {
                            media_uuid: msg.media_uuid,
                            uid: uid,
                            title: msg.title,
                            timestamp: Local::now().timestamp(),
                            resolved: false,
                            comments: HashMap::new(),
                        },
                    }
                    .into(),
                )
                .await
                .context("Failed to send CreateTicket message")?;

            let result = rx
                .await
                .context("Failed to receive CreateTicket response")??;

            Ok(Json(CreateTicketResp {
                ticket_uuid: result,
            })
            .into_response())
        }
        TicketMessage::CreateComment(msg) => {
            // auth
            //
            // anyone who can see a ticket may attach a comment
            if !state.can_access_ticket(&uid, &msg.ticket_uuid).await? {
                return Ok(StatusCode::UNAUTHORIZED.into_response());
            }

            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .send(
                    DbMsg::CreateComment {
                        resp: tx,
                        comment: TicketComment {
                            ticket_uuid: msg.ticket_uuid,
                            uid: uid,
                            text: msg.comment_text,
                            timestamp: Local::now().timestamp(),
                        },
                    }
                    .into(),
                )
                .await
                .context("Failed to send CreateComment message")?;

            let result = rx
                .await
                .context("Failed to receive CreateComment response")??;

            Ok(Json(CreateCommentResp {
                comment_uuid: result,
            })
            .into_response())
        }
        TicketMessage::GetTicket(msg) => {
            // auth
            //
            // anyone who can see a ticket may fetch it
            if !state.can_access_ticket(&uid, &msg.ticket_uuid).await? {
                return Ok(StatusCode::UNAUTHORIZED.into_response());
            }

            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .send(
                    DbMsg::GetTicket {
                        resp: tx,
                        ticket_uuid: msg.ticket_uuid,
                    }
                    .into(),
                )
                .await
                .context("Failed to send GetTicket message")?;

            match rx.await.context("Failed to receive GetTicket response")?? {
                Some(result) => Ok(Json(GetTicketResp { ticket: result }).into_response()),
                None => Ok(StatusCode::NOT_FOUND.into_response()),
            }
        }
        TicketMessage::SetTicketResolved(msg) => {
            // auth
            //
            // the ticket owner may update the resolved status
            if !state.owns_ticket(&uid, &msg.ticket_uuid).await? {
                return Ok(StatusCode::UNAUTHORIZED.into_response());
            }

            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .send(
                    DbMsg::SetTicketResolved {
                        resp: tx,
                        ticket_uuid: msg.ticket_uuid,
                        resolved: msg.resolved,
                    }
                    .into(),
                )
                .await
                .context("Failed to send SetTicketResolved message")?;

            rx.await
                .context("Failed to receive SetTicketResolved response")??;

            Ok(Json(SetTicketResolvedResp {}).into_response())
        }
        TicketMessage::TicketSearch(msg) => {
            // auth
            //
            // handled as part of the search query
            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .send(
                    DbMsg::SearchTickets {
                        resp: tx,
                        user: uid,
                        filter: msg.filter,
                        resolved: msg.resolved,
                    }
                    .into(),
                )
                .await
                .context("Failed to send SearchTickets message")?;

            let result = rx
                .await
                .context("Failed to receive SearchTickets respsonse")??;

            Ok(Json(SearchTicketsResp { tickets: result }).into_response())
        }
    }
}
