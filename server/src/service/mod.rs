pub mod msg;

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use anyhow;

use async_trait::async_trait;

use tokio;

// config
pub struct ESConfig {}

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
    Auth(crate::auth::msg::AuthMsg),
    Db(crate::db::msg::DbMsg),
    Fs(crate::fs::msg::FsMsg),
    Http(crate::http::msg::HttpMsg),
    Svc(crate::service::msg::Svc),
}

#[derive(Debug, Eq, Hash, PartialEq)]
pub enum ServiceType {
    Auth,
    Db,
    Fs,
    Http,
}

#[async_trait]
pub trait EntanglementService: Send + Sync + 'static {
    type Inner: ESInner;

    fn create(config: Arc<ESConfig>) -> (ESMSender, Self);

    async fn start(&self, senders: HashMap<ServiceType, ESMSender>) -> anyhow::Result<()>;
}

#[async_trait]
pub trait ESInner: Sized + Send + Sync + 'static {
    fn new(senders: HashMap<ServiceType, ESMSender>) -> anyhow::Result<Self>;

    async fn message_handler(&self, esm: ESM) -> anyhow::Result<()>;

    // rather than have the inner service trait functions (i.e., the RPC calls) respond directly,
    // we define this helper function for use in the message_handler loop
    //
    // this is necessary so that the RPC functions can be used by each other without any weird
    // Option<resp> or the like
    //
    // note that the type_name is current best effort at providing a bit more information, but it
    // will likely go away with a real logging setup
    async fn respond<T, Fut>(&self, resp: ESMResp<T>, fut: Fut) -> anyhow::Result<()>
    where
        T: Send + Sync,
        Fut: Future<Output = anyhow::Result<T>> + Send,
    {
        resp.send(fut.await).map_err(|_| {
            anyhow::Error::msg(format!(
                "failed to respond to a {} message",
                std::any::type_name::<T>()
            ))
        })
    }
}
