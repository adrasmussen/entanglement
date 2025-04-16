use std::{future::Future, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use dashmap::DashMap;
use tokio;

use common::config::ESConfig;

// these are the services that make up the entanglment server backend
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ServiceType {
    Auth,
    Db,
    Http,
    Task,
}

// Entanglement Service Messages
//
// without higher-kinded types, we use the normal enum-of-enums
// to enable general safe message passing between services
pub type ESMSender = tokio::sync::mpsc::Sender<ESM>;
pub type ESMReceiver = tokio::sync::mpsc::Receiver<ESM>;

// message responses are carried back via oneshot channels.  this
// type eliminates quite a bit of boilerplate in the responder logic.
pub type ESMResp<T> = tokio::sync::oneshot::Sender<Result<T>>;

#[derive(Debug)]
pub enum ESM {
    Auth(crate::auth::msg::AuthMsg),
    Db(crate::db::msg::DbMsg),
    _Http(crate::http::msg::HttpMsg),
    Task(crate::task::msg::TaskMsg),
}

// service registry
//
// currently, we assume that each service will be instantiated once, and that there
// should be one message namespace.  for this project, these are not terribly onerous
// requirements, and it simplifies generic service traits via get_registry().
//
// however, many services avoid the hash table lookup by cloning the sender, so care
// needs to be taken if this struct becomes dynamic in some fashion.
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
                    "internal error: a sender was added twice to the registry",
                ));
            }
        }
    }

    pub fn get(&self, k: &ServiceType) -> Result<ESMSender> {
        Ok(self
            .0
            .get(k)
            .ok_or_else(|| {
                anyhow::Error::msg(format!(
                    "internal error: a service was started without a necessary dependency ({:?})",
                    k
                ))
            })?
            .clone())
    }
}

// core service trait
//
// curiously enough, with some work we may be able to eliminate this trait
// altogether, since the ESInner abstraction holds basically all of the
// interesting state and leaves the outer part with just the registery,
// senders, and handles.
#[async_trait]
pub trait EntanglementService: Send + Sync + 'static {
    type Inner: ESInner;

    fn create(config: Arc<ESConfig>, registry: &ESMRegistry) -> Self;

    async fn start(&self, registry: &ESMRegistry) -> Result<()>;
}

// service message responder
//
// in the spirit of tower, the magic of the entanglement service model is in the message_handler
// rpc function.  services may respond to extneral messages on other channels (http) as well.
#[async_trait]
pub trait ESInner: Sized + Send + Sync + 'static {
    fn new(config: Arc<ESConfig>, registry: ESMRegistry) -> Result<Self>;

    fn registry(&self) -> ESMRegistry;

    async fn message_handler(&self, esm: ESM) -> Result<()>;

    // rather than have the inner service trait functions (i.e., the rpc calls) respond directly,
    // we define this helper function for use in the message_handler loop
    //
    // this is necessary so that the rpc functions can be used by each other without any weird
    // Option<resp> or the like
    //
    // TODO -- replace type_name with tracing logic, which almost certainly requires T: Debug at
    // least; it may be worth it to have T: Display instead and write an impl for the whole enum
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
