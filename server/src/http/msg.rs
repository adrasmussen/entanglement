#[derive(Debug)]
pub enum HttpMsg {
    _Status,
    // possible method to stop messages from piling up
    // when the auth service has an external issue
    _AuthProviderFailure,
}
