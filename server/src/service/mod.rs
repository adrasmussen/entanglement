use anyhow;

use tokio;

// Entanglement Service Messages
//
// without higher-kinded types, we use the normal enum-of-enums
// to enable general safe message passing between services
pub type ESMSender = tokio::sync::mpsc::Sender<ESM>;
pub type ESMReceiver = tokio::sync::mpsc::Receiver<ESM>;

// message responses are carried back via oneshot channels, and
// this ensures consistency
pub type ESMResp<T> = tokio::sync::oneshot::Sender<anyhow::Result<T>>;

#[derive(Debug)]
pub enum ESM {
    Svc(Svc),
    Db(crate::db::msg::DbMsg),
    Http(crate::http::msg::HttpMsg),
    Fs(crate::fs::msg::FsMsg),
}

// we will likely just have main.rs set everything up w/o a dedicated service manager
#[derive(Debug)]
pub enum Svc {
    _Status,
    _Add,
    _Start,
    _Stop,
    _GetSender,
    _Ping,
}
