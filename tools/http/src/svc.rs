use std::{
    net::{SocketAddr, SocketAddrV6},
    sync::Arc,
};

use anyhow::Result;
use axum::{
    Router,
    extract::Extension,
    http::{HeaderMap, HeaderName},
    response::IntoResponse,
    routing::get,
};
use hyper::{
    Request, StatusCode, body::Incoming, server::conn::http2::Builder, service::service_fn,
};
use hyper_util::rt::{TokioExecutor, TokioIo};
use rustls::{RootCertStore, ServerConfig, server::WebPkiClientVerifier};
use rustls_pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tower::Service;
use x509_certificate::X509Certificate;

use crate::ConnMode;
use api::HTTP_URL_ROOT;
use common::config::ESConfig;

pub async fn serve_http(config: Arc<ESConfig>, mode: &ConnMode) -> ! {
    let config = config.clone();
    let mode = mode.clone();

    let socket = SocketAddr::from(
        config
            .http
            .socket
            .parse::<SocketAddrV6>()
            .expect("Failed to parse http_socket ipv6 address/port"),
    );

    let app_url_root = HTTP_URL_ROOT.to_owned();

    let router = if mode == ConnMode::Proxy {
        let proxy_auth_config = config
            .proxyheader
            .clone()
            .expect("proxy header config missing");

        let header_key =
            HeaderName::from_lowercase(proxy_auth_config.header.to_lowercase().as_bytes())
                .expect("invalid proxy auth header");

        let proxy_cn = proxy_auth_config.proxy_cn;

        let service = async move |headers: HeaderMap, Extension(peer_cn): Extension<String>| {
            let proxy_user = match headers.get(&header_key) {
                Some(val) => match val.to_str() {
                    Ok(val) => val.to_owned(),
                    Err(_) => {
                        println!("failed to convert proxy header value to string");
                        return (
                            StatusCode::BAD_REQUEST,
                            "failed to convert proxy header value".to_owned(),
                        )
                            .into_response();
                    }
                },
                None => {
                    println!("proxy header missing!");
                    return (StatusCode::BAD_REQUEST, "proxy header missing".to_owned())
                        .into_response();
                }
            };

            if peer_cn != proxy_cn {
                println!("peer cn does not match config file");
                return (
                    StatusCode::BAD_REQUEST,
                    "peer cn does not match config file".to_owned(),
                )
                    .into_response();
            }

            println!("proxy cn: {peer_cn}");
            println!("proxy header: {header_key}");
            println!("proxy user: {proxy_user}");

            (
                StatusCode::OK,
                "entanglement proxy connection check succeeded".to_owned(),
            )
                .into_response()
        };

        Router::<()>::new().route(&format!("/{app_url_root}/check"), get(service))
    } else {
        let service = async move |_headers: HeaderMap, Extension(peer_cn): Extension<String>| {
            println!("user cn: {peer_cn}");

            (
                StatusCode::OK,
                "entanglement proxy connection check succeeded".to_owned(),
            )
                .into_response()
        };

        Router::<()>::new().route(&format!("/{app_url_root}/check"), get(service))
    };

    // tls setup
    let key: PrivateKeyDer =
        PemObject::from_pem_file(&config.http.key).expect("http server failed to load key");

    let cert: Vec<CertificateDer> = CertificateDer::pem_file_iter(&config.http.cert)
        .expect("http server failed to read cert")
        .collect::<Result<Vec<_>, _>>()
        .expect("http server failed to load cert");

    let mut client_ca_root_store = RootCertStore::empty();

    let client_ca_path = config
        .http
        .client_ca_cert
        .clone()
        .expect("http server configured for proxy header auth but no client ca cert specified");

    let ca_cert: Vec<CertificateDer> = CertificateDer::pem_file_iter(client_ca_path)
        .expect("http server failed to read client ca cert")
        .collect::<Result<Vec<_>, _>>()
        .expect("http server failed to load client ca cert");

    for c in ca_cert {
        client_ca_root_store
            .add(c)
            .expect("http server failed to add client ca cert to root store")
    }

    let client_cert_verifier = WebPkiClientVerifier::builder(client_ca_root_store.into())
        .build()
        .expect("http server failed to set up client verifier");

    let mut tls_config = ServerConfig::builder()
        .with_client_cert_verifier(client_cert_verifier)
        .with_single_cert(cert, key)
        .expect("http server failed to configure tls");

    tls_config.alpn_protocols = vec!["h2".into()];

    // set up the socket
    let listener = TcpListener::bind(socket)
        .await
        .expect("http listener failed to bind tcp socket");

    let tls_acceptor = TlsAcceptor::from(Arc::new(tls_config));

    loop {
        let router = router.clone();

        match listener.accept().await {
            Ok((stream, addr)) => {
                println!("accepting connection from {addr}");
                match tls_acceptor.accept(stream).await {
                    Ok(stream) => {
                        println!("connected over tls");

                        let (_io, server_session) = stream.get_ref();

                        let certs = match server_session.peer_certificates() {
                            Some(val) => val,
                            None => {
                                println!("error getting peer certificates");
                                continue;
                            }
                        };

                        let cert = match certs.first() {
                            Some(val) => match X509Certificate::from_der(val) {
                                Ok(cert) => cert,
                                Err(err) => {
                                    println!("error converting peer certificate: {err:#?}");
                                    continue;
                                }
                            },
                            None => {
                                println!("error finding first peer certificate");
                                continue;
                            }
                        };

                        let peer_cn = match cert.subject_common_name() {
                            Some(val) => val,
                            None => {
                                println!("no peer subject common name");
                                continue;
                            }
                        };

                        println!("peer common name: {peer_cn}");
                        println!("protocol version: {:?}", server_session.protocol_version());
                        println!(
                            "cipher suite: {:?}",
                            server_session.negotiated_cipher_suite().map(|c| c.suite())
                        );
                        println!("alpn protocol: {:?}", server_session.alpn_protocol());

                        let service = service_fn(move |mut request: Request<Incoming>| {
                            let peer_cn = peer_cn.clone();

                            request.extensions_mut().insert(peer_cn);

                            router.clone().call(request)
                        });

                        let io = TokioIo::new(stream);

                        let res = Builder::new(TokioExecutor::new())
                            .serve_connection(io, service)
                            .await;

                        match res {
                            Ok(_) => {
                                println!("finished serving connection")
                            }
                            Err(err) => {
                                println!("error serving connection: {err:#?}")
                            }
                        }
                    }
                    Err(err) => {
                        println!("error initializing tls: {err:#?}")
                    }
                }
            }

            Err(err) => {
                println!("error accepting initial stream: {err:#?}")
            }
        };
    }
}
