use std::future::Future;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use dashmap::DashMap;
use tokio;

use common::config::ESConfig;

pub mod msg;

// these are the services that make up the entanglment server backend
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ServiceType {
    Auth,
    Db,
    Fs,
    Http,
    Task,
}

// Entanglement Service Messages
//
// without higher-kinded types, we use the normal enum-of-enums
// to enable general safe message passing between services
pub type ESMSender = tokio::sync::mpsc::Sender<ESM>;
pub type ESMReceiver = tokio::sync::mpsc::Receiver<ESM>;

// message responses are carried back via oneshot channels, and
// this ensures consistency
pub type ESMResp<T> = tokio::sync::oneshot::Sender<Result<T>>;

#[derive(Debug)]
pub enum ESM {
    Auth(crate::auth::msg::AuthMsg),
    Db(crate::db::msg::DbMsg),
    Fs(crate::fs::msg::FsMsg),
    _Http(crate::http::msg::HttpMsg),
    _Svc(crate::service::msg::Svc),
    Task(crate::task::msg::TaskMsg),
}

// currently, we assume that each service will be instantiated
// once, and that there should be one message namespace
//
// i'm not super happy with this whole system, but it's easy to
// understand and reasonably performant
#[derive(Clone, Debug)]
pub struct ESMRegistry(Arc<DashMap<ServiceType, ESMSender>>);

impl ESMRegistry {
    pub fn new() -> Self {
        ESMRegistry(Arc::new(DashMap::new()))
    }

    pub fn insert(&self, k: ServiceType, v: ESMSender) -> Result<()> {
        match self.0.clone().insert(k.clone(), v) {
            None => return Ok(()),
            Some(w) => {
                self.0.clone().insert(k, w);
                return Err(anyhow::Error::msg(
                    "internal compile time error -- a sender was added twice to the registry",
                ));
            }
        }
    }

    pub fn get(&self, k: &ServiceType) -> Result<ESMSender> {
        Ok(self.0.get(k).ok_or_else(|| anyhow::Error::msg(format!("internal compile time error -- a service was started without a necessary dependency ({:?})", k)))?.clone())
    }
}

#[async_trait]
pub trait EntanglementService: Send + Sync + 'static {
    type Inner: ESInner;

    fn create(config: Arc<ESConfig>, registry: &ESMRegistry) -> Self;

    async fn start(&self, registry: &ESMRegistry) -> Result<()>;
}

#[async_trait]
pub trait ESInner: Sized + Send + Sync + 'static {
    fn new(config: Arc<ESConfig>, registry: ESMRegistry) -> Result<Self>;

    fn registry(&self) -> ESMRegistry;

    async fn message_handler(&self, esm: ESM) -> Result<()>;

    // rather than have the inner service trait functions (i.e., the RPC calls) respond directly,
    // we define this helper function for use in the message_handler loop
    //
    // this is necessary so that the RPC functions can be used by each other without any weird
    // Option<resp> or the like
    //
    // note that the type_name is current best effort at providing a bit more information, but it
    // will likely go away with a real logging setup
    async fn respond<T, Fut>(&self, resp: ESMResp<T>, fut: Fut) -> Result<()>
    where
        T: Send + Sync,
        Fut: Future<Output = Result<T>> + Send,
    {
        resp.send(fut.await).map_err(|_| {
            anyhow::Error::msg(format!(
                "failed to respond to a {} message",
                std::any::type_name::<T>()
            ))
        })
    }
}
