use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};

// the header set by the reverse proxy that we will implicitly trust
const PROXY_AUTH_HEADER: &str = "Proxy-User";

// user auth information passed in from middleware to the axum extractors
#[derive(Clone)]
pub struct CurrentUser {
    pub uid: String,
}

pub async fn proxy_auth(mut req: Request, next: Next) -> Result<Response, StatusCode> {
    // attempt to unpack the auth header, returning None if we cannot convert to a str
    let auth_header = req
        .headers()
        .get(http::header::HeaderName::from_static(PROXY_AUTH_HEADER))
        .and_then(|header| header.to_str().ok());

    let auth_header = match auth_header {
        Some(val) => val,
        None => return Err(StatusCode::UNAUTHORIZED),
    };

    let user = CurrentUser {
        uid: auth_header.to_owned(),
    };

    // if auth succeeds, pass CurrentUser as a request extension to handlers
    req.extensions_mut().insert(user);

    // then, continue on in the tower of middleware
    Ok(next.run(req).await)
}

async fn _password_auth(mut req: Request, next: Next) -> Result<Response, StatusCode> {
    // attempt to unpack the auth header, returning None if we cannot convert to a str
    let auth_header = req
        .headers()
        .get(http::header::AUTHORIZATION)
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

// async fn handler(
//     // extract the current user, set by the middleware
//     Extension(current_user): Extension<CurrentUser>,
// ) {
//     // ...
// }

// let app = Router::new()
//     .route("/", get(handler))
//     .route_layer(middleware::from_fn(auth));
