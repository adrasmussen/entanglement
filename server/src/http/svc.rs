use std::sync::Arc;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::PathBuf;

use anyhow;

use axum::{
    body::Bytes,
    extract::{Extension, Request},
    http::{StatusCode, Uri},
    middleware,
    routing::{get, post},
    Router,
};

use futures_util::TryStreamExt;

use http_body_util::{combinators::BoxBody, BodyExt, StreamBody};

use hyper::body::{Frame, Incoming};

use hyper_util;

use tokio;

use tower::Service;

use crate::http::auth::{proxy_auth, CurrentUser};
use crate::service::{ESConfig, ESMReceiver, ESMSender, EntanglementService, ESM};

pub struct HttpService {
    config: Arc<ESConfig>,
    sender: ESMSender,
    receiver: ESMReceiver,
    // state: ???
}

// HttpService message handler
async fn message_handler(esm: ESM) -> anyhow::Result<()> {
    match esm {
        ESM::Http(http_msg) => match http_msg {
            _ => Err(anyhow::Error::msg("not implmented")),
        },
        _ => Err(anyhow::Error::msg("not implmented")),
    }
}


















// can read files directly, mapping /api/image/<sha> to /path/to/library/.../img.jpg,
// checking permissions along the way (i.e. user can see an album containing image)

async fn serve_http() -> Result<(), anyhow::Error> {
    let addr = SocketAddr::from(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8081));

    let router: Router<()> = Router::new()
        .route("/api/match", post(generate_filter))
        .route("/api/album", post(edit_album))
        .route("/api/status", get(status))
        .route("/api/images/*file", get(download_file))
        .route("/api/thumbs/*file", get(download_file))
        .fallback(fallback)
        .route_layer(middleware::from_fn(proxy_auth));

    let service =
        hyper::service::service_fn(move |request: Request<Incoming>| router.clone().call(request));

    // for the moment, we just fail if the socket is in use
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

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

async fn generate_filter() {}

async fn edit_album() {}

async fn status() {}

async fn download_file(
    Extension(current_user): Extension<CurrentUser>,
    axum::extract::Path(file): axum::extract::Path<String>,
) {
}

// async fn download_file(axum::extract::Path(file): axum::extract::Path<String>) -> hyper::Response<BoxBody<tokio_util::bytes::Bytes, std::io::Error>> {
//     // where the files live
//     let webroot = PathBuf::from("/srv/home/alex/webroot");

//     // translate the URI to a real file
//     let filename = webroot.join(PathBuf::from(file));

//     let file_handle = match tokio::fs::File::open(filename).await {
//         Ok(f) => f,
//         Err(err) => {
//             return hyper::Response::builder()
//                 .status(StatusCode::NOT_FOUND)
//                 .body(string_to_boxedbody(err.to_string()))
//                 .unwrap()
//         }
//     };

//     // following hyper's send_file example
//     let reader_stream = tokio_util::io::ReaderStream::new(file_handle);

//     // convert to the boxed streaming body
//     let stream_body = StreamBody::new(reader_stream.map_ok(Frame::data));
//     let boxed_body = stream_body.boxed();

//     hyper::Response::builder()
//         .status(StatusCode::OK)
//         .header("Access-Control-Allow-Origin", "*")
//         .body(boxed_body)
//         .unwrap()

// }
