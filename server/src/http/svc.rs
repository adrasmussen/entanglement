use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{self, Context};

use async_cell::sync::AsyncCell;

use async_trait::async_trait;

use axum::{
    extract::{Extension, Json, Path, Query, Request, State},
    http::{StatusCode, Uri},
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};

use chrono::offset::Local;

use serde::{Deserialize, Serialize};

use tokio::sync::Mutex;

use tokio_util::io::ReaderStream;

use tower::Service;

use crate::auth::msg::AuthMsg;
use crate::db::msg::DbMsg;
use crate::http::{
    auth::{proxy_auth, CurrentUser},
    AppError,
};
use crate::service::*;
use api::{album::*, image::*, ticket::*, *};

#[derive(Clone, Debug)]
pub struct HttpEndpoint {
    auth_svc_sender: ESMSender,
    db_svc_sender: ESMSender,
    fs_svc_sender: ESMSender,
    media_linkdir: PathBuf,
}

impl HttpEndpoint {
    async fn can_access_media(&self, uid: &String, media_uuid: &MediaUuid) -> anyhow::Result<bool> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.auth_svc_sender
            .clone()
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
            .clone()
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

    // same for these two
    async fn can_access_album(&self, uid: &String, album_uuid: &AlbumUuid) -> anyhow::Result<bool> {
        todo!()
    }

    async fn owns_album(&self, uid: &String, album_uuid: &AlbumUuid) -> anyhow::Result<bool> {
        todo!()
    }

    // this can likely be cached somehow -- move to Auth service since we can cache the group info
    async fn can_access_ticket(
        &self,
        user: &String,
        ticket_uuid: &TicketUuid,
    ) -> anyhow::Result<bool> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.db_svc_sender
            .clone()
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
            .context("Failed to receive GetTicket response at HttpEndpoint")??;

        self.can_access_media(&user, &ticket.media_uuid).await
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
    sender: ESMSender,
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
            tx.clone(),
            HttpService {
                config: config.clone(),
                sender: tx,
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
        let socket = SocketAddr::from(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8081));

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

    let router: Router<()> = Router::new()
        .route("/media/:uuid", get(stream_media))
        .route("/api/search", post(search_media))
        .route("/api/user", post(query_user))
        .route("/api/group", post(query_group))
        .route("/api/album", post(query_album))
        .route("/api/ticket", post(query_ticket))
        .fallback(fallback)
        .route_layer(middleware::from_fn(proxy_auth))
        .with_state(state);

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

async fn fallback(uri: Uri) -> StatusCode {
    StatusCode::NOT_FOUND
}

async fn stream_media(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Path(uuid): Path<i64>,
) -> Result<Response, AppError> {
    let state = state.clone();

    // first determine if the user can access the file
    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .auth_svc_sender
        .clone()
        .send(
            AuthMsg::CanAccessMedia {
                resp: tx,
                uid: current_user.user.clone(),
                media_uuid: uuid.clone(),
            }
            .into(),
        )
        .await
        .context("Failed to send CanAccessMedia message.")?;

    let allowed = rx
        .await
        .context("Failed to receive CanAccessMedia response")??;

    if !allowed {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    // once we've passed the auth check, we get the file handle and build
    // a streaming body from it
    let filename = state.media_linkdir.join(uuid.to_string());

    let file_handle = match tokio::fs::File::open(filename).await {
        Ok(f) => f,
        Err(err) => return Ok((StatusCode::NOT_FOUND, err.to_string()).into_response()),
    };

    let reader_stream = ReaderStream::new(file_handle);

    Ok(axum::body::Body::from_stream(reader_stream).into_response())
}

async fn search_media(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(json_body): Json<MediaSearchReq>,
) -> Result<Response, AppError> {
    let state = state.clone();

    let user = current_user.user.clone();

    let filter = json_body.filter;

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .clone()
        .send(
            DbMsg::SearchMedia {
                resp: tx,
                user: user,
                filter: filter,
            }
            .into(),
        )
        .await
        .context("Failed to send SearchImage message")?;

    let result = rx
        .await
        .context("Failed to receive SearchImage response")??;

    Ok(Json(MediaSearchResp { media: result }).into_response())
}

// need to set up a query here to handle each use case

async fn query_user(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Query(query): Query<(String)>,
    json_body: Json<()>,
) -> Response {
    StatusCode::IM_A_TEAPOT.into_response()
}

async fn query_group(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Path(op): Path<(String)>,
    json_body: Json<()>,
) -> Response {
    StatusCode::IM_A_TEAPOT.into_response()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AlbumMessage {
    CreateAlbum(CreateAlbumReq),
    GetAlbum(GetAlbumReq),
    DeleteAlbum(DeleteAlbumReq),
    UpdateAlbum(UpdateAlbumReq),
    AddMediaToAlbum(AddMediaToAlbumReq),
    RmMediaFromAlbum(RmMediaFromAlbumReq),
    SearchAlbums(SearchAlbumsReq),
    SearchMediaInAlbum(SearchMediaInAlbumReq),
}

async fn query_album(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<AlbumMessage>,
) -> Result<Response, AppError> {
    let state = state.clone();

    let user = current_user.user.clone();

    match message {
        AlbumMessage::CreateAlbum(msg) => {
            // auth
            //
            // anyone may create an album
            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .clone()
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
        _ => Ok(StatusCode::IM_A_TEAPOT.into_response()),
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TicketMessage {
    CreateTicket(CreateTicketReq),
    CreateComment(CreateCommentReq),
    GetTicket(GetTicketReq),
    TicketSearch(SearchTicketsReq),
}

async fn query_ticket(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Json(message): Json<TicketMessage>,
) -> Result<Response, AppError> {
    let state = state.clone();

    let user = current_user.user.clone();

    match message {
        TicketMessage::CreateTicket(msg) => {
            // auth
            //
            // anyone who can see media can create a new ticket, even if they don't own it
            if !state.can_access_media(&user, &msg.media_uuid).await? {
                return Ok(StatusCode::UNAUTHORIZED.into_response());
            }

            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .clone()
                .send(
                    DbMsg::CreateTicket {
                        resp: tx,
                        ticket: Ticket {
                            media_uuid: msg.media_uuid,
                            owner: user,
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
            if !state.can_access_ticket(&user, &msg.ticket_uuid).await? {
                return Ok(StatusCode::UNAUTHORIZED.into_response());
            }

            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .clone()
                .send(
                    DbMsg::CreateComment {
                        resp: tx,
                        comment: TicketComment {
                            ticket_uuid: msg.ticket_uuid,
                            owner: user,
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
            // it's awkward that we effectively call get_ticket twice, but this ensures
            // a consistent auth pattern between various calls
            if !state.can_access_ticket(&user, &msg.ticket_uuid).await? {
                return Ok(StatusCode::UNAUTHORIZED.into_response());
            }

            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .clone()
                .send(
                    DbMsg::GetTicket {
                        resp: tx,
                        ticket_uuid: msg.ticket_uuid,
                    }
                    .into(),
                )
                .await
                .context("Failed to send GetTicket message")?;

            let result = rx.await.context("Failed to receive GetTicket response")??;

            Ok(Json(GetTicketResp { ticket: result }).into_response())
        }
        TicketMessage::TicketSearch(msg) => {
            // auth
            //
            // like all search methods, auth is handled as part of the db query

            let (tx, rx) = tokio::sync::oneshot::channel();

            state
                .db_svc_sender
                .clone()
                .send(
                    DbMsg::SearchTickets {
                        resp: tx,
                        user: user,
                        filter: msg.filter,
                        resolved: msg.resolved,
                    }
                    .into(),
                )
                .await
                .context("Failed to send SearchTickets message");

            let result = rx
                .await
                .context("Failed to receive SearchTickets respsonse")??;

            Ok(Json(SearchTicketsResp { tickets: result }).into_response())
        }
    }
}
