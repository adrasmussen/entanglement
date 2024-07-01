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

// consider a universal service pattern that differs only on the State, and then
// have an impl for specific internal states
//
// in this model, the message_handler is semi-programatically defined by the
// messages themselves, tied to the State struct instead of the server wrapper
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

#[async_trait]
pub trait EntanglementService: Send + Sync + 'static {
    type State: Send + Sync + 'static;

    fn create(config: Arc<ESConfig>) -> (ESMSender, Self);

    async fn start(&self, senders: HashMap<ESM, ESMSender>) -> anyhow::Result<()>;
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
