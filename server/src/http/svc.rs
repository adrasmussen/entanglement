use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{self, Context};

use async_cell::sync::AsyncCell;

use async_trait::async_trait;

use axum::{
    extract::{Extension, Json, Path, Request, State},
    http::{StatusCode, Uri},
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};

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
use api::{image::*, MediaUuid};

#[derive(Clone, Debug)]
pub struct HttpEndpoint {
    auth_svc_sender: ESMSender,
    db_svc_sender: ESMSender,
    fs_svc_sender: ESMSender,
    media_linkdir: PathBuf,
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
    Path(uuid): Path<u64>,
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
                uuid: uuid.clone(),
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
    Json(json_body): Json<ImageSearchReq>,
) -> Result<Response, AppError> {
    let state = state.clone();

    let user = current_user.user.clone();

    let filter = json_body.filter;

    let (tx, rx) = tokio::sync::oneshot::channel();

    state
        .db_svc_sender
        .clone()
        .send(
            DbMsg::SearchImages {
                resp: tx,
                user: user,
                filter: filter,
            }
            .into(),
        )
        .await
        .context("Failed to send SearchImage message")?;

    let result = rx.await.context("Failed to receive SearchImage response")??;

    Ok(Json(ImageSearchResp {images: result}).into_response())
}

// need to set up a query here to handle each use case

async fn query_user(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Path(op): Path<(String)>,
    json_body: Json<ImageSearchReq>,
) -> Response {
    StatusCode::IM_A_TEAPOT.into_response()
}

async fn query_group(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Path(op): Path<(String)>,
    json_body: Json<ImageSearchReq>,
) -> Response {
    StatusCode::IM_A_TEAPOT.into_response()
}

async fn query_album(
    State(state): State<Arc<HttpEndpoint>>,
    Extension(current_user): Extension<CurrentUser>,
    Path(op): Path<(String)>,
    json_body: Json<ImageSearchReq>,
) -> Response {
    StatusCode::IM_A_TEAPOT.into_response()
}
