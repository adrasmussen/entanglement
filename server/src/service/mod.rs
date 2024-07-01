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
    Svc(crate::service::msg::Svc),
    Db(crate::db::msg::DbMsg),
    Http(crate::http::msg::HttpMsg),
    Fs(crate::fs::msg::FsMsg),
}

// config
pub struct ESConfig {}

#[async_trait]
pub trait EntanglementService: Send + Sync + 'static {
    fn create(config: Arc<ESConfig>) -> (ESMSender, Arc<Self>);

    async fn message_handler(self: Arc<Self>, esm: ESM) -> anyhow::Result<()>;

    async fn start(self: Arc<Self>, senders: HashMap<ESM, ESMSender>) -> anyhow::Result<tokio::task::JoinHandle<anyhow::Result<()>>>;
}

// we need all of this artifice because the message_handler subfunctions might
// mutate the state of the service, or at least depend on its internals
//
// unfortunately, we're being lazy and passing in the whole service struct
// instead of just
macro_rules! handler_loop {
    ($svc_name:literal) => {
        let serve = {
            let self = Arc::clone(self);
            async move {
                while let Some(msg) = self.clone().receiver.lock().await.recv().await {
                    let self = Arc::clone(self);
                    tokio::task::spawn(async move {
                        match self.clone().message_handler(msg).await {
                            Ok(()) => (),
                            Err(err) => self.log(err.to_string()),
                        }
                    });
                }

                Err(anyhow::Error::msg(format!("{} channel disconnected", $svc_name)))
            }
        };
    };
}
