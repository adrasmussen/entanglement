use std::{
    net::{SocketAddr, SocketAddrV6},
    path::PathBuf,
    sync::Arc,
};

use anyhow::Result;
use async_cell::sync::AsyncCell;
use async_trait::async_trait;
use axum::{
    extract::Request,
    middleware,
    response::Redirect,
    routing::{get, post},
    Router,
};
use hyper::{body::Incoming, service::service_fn};
use hyper_util::{server::conn::auto::Builder, rt::{TokioExecutor, TokioIo}};
use regex::Regex;
use tokio::{
    net::TcpListener,
    sync::Mutex,
    task::{spawn, JoinHandle},
};
use tower::Service;
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use tracing::{debug, error, info, instrument, warn};

use crate::{
    auth::check::AuthCheck,
    http::{api::*, auth::proxy_auth, stream::*},
    service::{
        ESInner, ESMReceiver, ESMRegistry, ESMSender, EntanglementService, ServiceType, ESM,
    },
};
use api::HTTP_URL_ROOT;
use common::config::ESConfig;

// http service
//
// the http service handles both api calls and streaming media, along
// with enforcing the authentication policies on those endpoints
//
// hyper and axum form the core of the service, and the outer struct
// collects the hyper handle alongside the ESM handle

pub struct HttpService {
    config: Arc<ESConfig>,
    receiver: Arc<Mutex<ESMReceiver>>,
    msg_handle: AsyncCell<JoinHandle<Result<()>>>,
    hyper_handle: AsyncCell<JoinHandle<Result<()>>>,
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
    async fn start(&self, registry: &ESMRegistry) -> Result<()> {
        info!("starting http service");

        let receiver = Arc::clone(&self.receiver);
        let state = Arc::new(HttpEndpoint::new(self.config.clone(), registry.clone())?);

        // if we wanted to support more socket types, we could spawn several listeners since they
        // don't have any relevant state and the message handlers are all fully concurrent
        let socket = SocketAddr::from(
            self.config
                .http
                .socket
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
    pub(super) config: Arc<ESConfig>,
    pub(super) registry: ESMRegistry,
    pub(super) auth_svc_sender: ESMSender,
    pub(super) db_svc_sender: ESMSender,
    pub(super) task_svc_sender: ESMSender,
    pub(super) range_regex: Arc<Regex>,
}

#[async_trait]
impl ESInner for HttpEndpoint {
    fn new(config: Arc<ESConfig>, registry: ESMRegistry) -> Result<Self> {
        Ok(HttpEndpoint {
            config: config.clone(),
            registry: registry.clone(),
            // panic if we can't find all of the necessary senders, since this is a
            // compile-time problem and not a runtime problem
            auth_svc_sender: registry.get(&ServiceType::Auth).unwrap().clone(),
            db_svc_sender: registry.get(&ServiceType::Db).unwrap().clone(),
            task_svc_sender: registry.get(&ServiceType::Task).unwrap().clone(),
            // changes in this regex have to be accompanied by changing the capture match
            // settings in stream.rs, or it will panic on every invocation
            range_regex: Arc::new(Regex::new(r"(\d*)-(\d*)")?),
        })
    }

    fn registry(&self) -> ESMRegistry {
        self.registry.clone()
    }

    // currently there are no useful messages for this service to respond to
    async fn message_handler(&self, esm: ESM) -> Result<()> {
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
    async fn serve_http(self: Arc<Self>, socket: SocketAddr) -> JoinHandle<Result<()>> {
        info!("starting axum/hyper http listener");

        let config = self.config.clone();
        let state = Arc::clone(&self);

        // http root
        //
        // this controls the root of the url for all entanglement http behavior, i.e.
        // the http://<ip:port>/<root>/... part prepended to all requests.
        //
        // ideally, this would be set via a commandline parameter, but Dioxus makes
        // that a bit tricky.  for now, we set it here and in Dioxus.toml, see the
        // definition in api/lib.rs.
        let app_url_root = HTTP_URL_ROOT.to_owned();

        // app -- the WASM webapp

        // this is the filesystem location of the /dist folder created by the dioxsus
        // build process
        //
        // it should be baked into a containerized runtime for the server, but does not
        // necessarily have to be so (it can be hosted outside)
        let app_web_dir = config.http.doc_root.clone();

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
        // middleware from the rest of the http module.  these must match the defitions used in the
        // api crate endpoint macro, link functions, and Dioxus.toml
        //
        // the fallback here is a bit weird, we need to be careful that it redirects properly with
        // the app router's own fallback.
        //
        // see also api/lib.rs for endpoint functions that depend on the nesting paths
        //
        // TODO -- generalize the auth middleware correctly
        let router = Router::new()
            .nest(&format!("/{app_url_root}/app"), app_router)
            .nest(&format!("/{app_url_root}/media"), media_router)
            .nest(&format!("/{app_url_root}/api"), api_router)
            .fallback(move || async move { Redirect::permanent(&format!("/{app_url_root}/app")) })
            .layer(TraceLayer::new_for_http())
            .route_layer(middleware::from_fn_with_state(
                config.proxyheader.clone().unwrap().header,
                proxy_auth,
            ));

        // everything from here follows the normal hyper/axum on tokio setup
        let service = service_fn(move |request: Request<Incoming>| router.clone().call(request));

        // for the moment, we just panic if the socket is in use
        let listener = TcpListener::bind(socket)
            .await
            .expect("http_service failed to bind tcp socket");

        // the main http server loop
        //
        // we want to return the handle to the caller, not the future, and so we just spawn it here
        let handle = spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let service = service.clone();

                let io = TokioIo::new(stream);

                spawn(async move {
                    match Builder::new(TokioExecutor::new())
                        .serve_connection(io, service.clone())
                        .await
                    {
                        Ok(()) => (),
                        Err(err) => {
                            // this is a hamfisted and abi-unstable way to filter out connection errors
                            if format!("{err:?}").contains("Io") {
                                debug!({service = "http_service", conn = "http", error = ?err})
                            } else {
                                warn!({service = "http_service", conn = "http", error = ?err})
                            }
                        }
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
