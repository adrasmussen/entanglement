use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{
        header::AUTHORIZATION,
        {HeaderName, StatusCode},
    },
    middleware::Next,
    response::Response,
};

use crate::http::svc::HttpEndpoint;

// user auth information passed in from middleware to the axum extractors,
// attached to the request via an extension with this type
#[derive(Clone)]
pub struct CurrentUser {
    pub uid: String,
}

// if the client provides a certificate as part of mutual tls, the subject
// cn field is put into a request extension in this type
#[derive(Clone)]
pub struct ClientCn {
    pub cn: String,
}

// authentication via reverse proxy
#[derive(Clone)]
pub struct ProxyAuthData {
    pub header_key: HeaderName,
    pub cn: String,
}

pub async fn proxy_auth(
    State(state): State<ProxyAuthData>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let proxy_cn = req
        .extensions()
        .get::<ClientCn>()
        .ok_or(StatusCode::UNAUTHORIZED)?;

    if proxy_cn.cn != state.cn {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // attempt to unpack the auth header, returning None if we cannot convert to a str
    let header_val = req
        .headers()
        .get(state.header_key)
        .and_then(|header| header.to_str().ok());

    let header_val = match header_val {
        Some(val) => val,
        None => return Err(StatusCode::UNAUTHORIZED),
    };

    let user = CurrentUser {
        uid: header_val.to_owned(),
    };

    // if auth succeeds, pass CurrentUser as a request extension to handlers
    req.extensions_mut().insert(user);

    // then, continue on in the tower of middleware
    Ok(next.run(req).await)
}

pub async fn cert_auth(mut req: Request, next: Next) -> Result<Response, StatusCode> {
    let user = CurrentUser {
        uid: req
            .extensions()
            .get::<ClientCn>()
            .ok_or(StatusCode::UNAUTHORIZED)?
            .cn
            .clone(),
    };

    // if auth succeeds, pass CurrentUser as a request extension to handlers
    req.extensions_mut().insert(user);

    // then, continue on in the tower of middleware
    Ok(next.run(req).await)
}

async fn _password_auth(
    State(_state): State<Arc<HttpEndpoint>>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // attempt to unpack the auth header, returning None if we cannot convert to a str
    let auth_header = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|header| header.to_str().ok());

    let auth_header = match auth_header {
        Some(val) => val,
        None => return Err(StatusCode::UNAUTHORIZED),
    };

    match _authorize(auth_header).await {
        Ok(user) => {
            // if auth succeeds, pass CurrentUser as a request extension to handlers
            req.extensions_mut().insert(user);

            // then, continue on in the tower of middleware
            Ok(next.run(req).await)
        }
        Err(_) => {
            // hypothetically, we could pass on further details here... or at least
            // log them in a sensible way
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

// eventually, this will send a message to the auth service
//
// note that we will have to un-base64 encode the data first...
async fn _authorize(_auth_token: &str) -> anyhow::Result<CurrentUser> {
    Err(anyhow::Error::msg("not implemented"))
}
