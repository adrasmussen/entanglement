mod db;
mod fs;
mod http;
mod service;



// everything that follows here is the temporary http implementation


use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::PathBuf;

use axum::{
    body::Bytes,
    http::StatusCode,
    routing::get,
    Router,
};

use futures_util::TryStreamExt;

use http_body_util::{combinators::BoxBody, BodyExt, StreamBody};

use hyper::{Request, body::{Frame, Incoming}};

use hyper_util;

use tokio;

use tower::{Service, ServiceBuilder};

// this might be the worst thing i have ever had the misfortune of writing
fn might_fail_iter(
    string: String,
    fail: bool,
) -> futures_util::stream::Iter<std::vec::IntoIter<Result<tokio_util::bytes::Bytes, std::io::Error>>>
{
    let old = string.into_bytes();

    let mut new = Vec::new();

    for s in old.into_iter() {
        if fail {
            new.push(Err(std::io::Error::new(std::io::ErrorKind::Other, "")));
        } else {
            new.push(Ok(Bytes::from(vec![s])));
        }
    }

    futures_util::stream::iter(new)
}

pub fn string_to_boxedbody(
    string: String,
) -> BoxBody<tokio_util::bytes::Bytes, std::io::Error> {
    let stream = might_fail_iter(string, false);

    http_body_util::StreamBody::new(stream.map_ok(Frame::data)).boxed()
}



async fn download_file(axum::extract::Path(file): axum::extract::Path<String>) -> hyper::Response<BoxBody<tokio_util::bytes::Bytes, std::io::Error>> {
    // where the files live
    let webroot = PathBuf::from("/srv/home/alex/webroot");

    // translate the URI to a real file
    let filename = webroot.join(PathBuf::from(file));

    let file_handle = match tokio::fs::File::open(filename).await {
        Ok(f) => f,
        Err(err) => {
            return hyper::Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(string_to_boxedbody(err.to_string()))
                .unwrap()
        }
    };

    // following hyper's send_file example
    let reader_stream = tokio_util::io::ReaderStream::new(file_handle);

    // convert to the boxed streaming body
    let stream_body = StreamBody::new(reader_stream.map_ok(Frame::data));
    let boxed_body = stream_body.boxed();

    hyper::Response::builder()
        .status(StatusCode::OK)
        .header("Access-Control-Allow-Origin", "*")
        .body(boxed_body)
        .unwrap()

}



#[tokio::main]
async fn main() {
    let addr = SocketAddr::from(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8081));

    let middleware = ServiceBuilder::new();

    let router: Router<()> = Router::new()
        .route("/api/*file", get(download_file))
        .layer(middleware);

    let service = hyper::service::service_fn(move |request: Request<Incoming>| {
        router.clone().call(request)
    });

    // for the moment, we just fail if the socket is in use
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    while let Ok((stream, _)) = listener.accept().await {
        let service = service.clone();

        let io = hyper_util::rt::TokioIo::new(stream);

        tokio::task::spawn(async move {
            match hyper_util::server::conn::auto::Builder::new(
                hyper_util::rt::TokioExecutor::new()
            ).serve_connection(io, service.clone()).await {
                Ok(()) => (),
                Err(err) => println!("{}", err.to_string())
            }
        });
    };
}
