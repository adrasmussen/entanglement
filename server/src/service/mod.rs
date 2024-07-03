pub mod msg;

use std::collections::HashMap;
use std::sync::Arc;

use anyhow;

use async_trait::async_trait;

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
    Cache(crate::cache::msg::CacheMsg),
    Db(crate::db::msg::DbMsg),
    Fs(crate::fs::msg::FsMsg),
    Http(crate::http::msg::HttpMsg),
    Svc(crate::service::msg::Svc),
}

// config
pub struct ESConfig {}

// consider a universal service pattern that differs only on the State, and then
// have an impl for specific internal states
//
// in this model, the message_handler is semi-programatically defined by the
// messages themselves, tied to the Inner struct instead of the server wrapper
//
// overall this is much cleaner (since it means the tokio channels are persistent)
// and does a better job enforcing the mutability of the state -- though we must
// check that the message handler uses an immutable reference for thread safety
//
// probably want an ESInnerState trait that sets the message handler details,
// maybe even going so far as to have Arc<Self> as the reciever
//
// also consider using tokio::select! on the server futures to check if the other
// threads have failed

// but... what about start?  should it start(&self) -> anyhow::Result<()> {state.start().await}

#[async_trait]
pub trait EntanglementService: Send + Sync + 'static {
    type Inner: ESInner;

    fn create(config: Arc<ESConfig>) -> (ESMSender, Self);

    async fn start(&self, senders: HashMap<ESM, ESMSender>) -> anyhow::Result<()>;
}

#[async_trait]
pub trait ESInner: Send + Sync + 'static {
    fn new() -> Self;

    async fn message_handler(&self, esm: ESM) -> anyhow::Result<()>;
}

// we need all of this artifice because the message_handler subfunctions might
// mutate the state of the service, or at least depend on its internals
//
// unfortunately, we're being lazy and passing in the whole service struct
// instead of just
macro_rules! handler_loop {
    ($svc_name:literal) => {
        let serve = {
            async move {
                while let Some(msg) = receiver.lock().await.recv().await {
                    let state = Arc::clone(&state);
                    tokio::task::spawn(async move {
                        match state.message_handler(msg).await {
                            Ok(()) => (),
                            Err(_) => println!("{} failed to reply to message", $svc_name),
                        }
                    });
                }

                Err::<(), anyhow::Error>(anyhow::Error::msg(format!(
                    "{} channel disconnected",
                    $svc_name
                )))
            }
        };
    };
}
