use std::collections::HashMap;
use std::sync::Arc;

use async_cell::sync::AsyncCell;

use async_trait::async_trait;

use tokio::sync::{Mutex, RwLock};

use api::Visibility;

use crate::service::*;

use crate::cache::msg::CacheMsg;
use crate::cache::ESCacheService;

pub struct Caches {
    image_visibility: Arc<RwLock<HashMap<String, Visibility>>>,
}

#[async_trait]
impl ESCacheService for Caches {
    async fn clear_all_caches(&self, resp: ESMResp<()>) -> anyhow::Result<()> {
        let inner = async { Ok(()) };

        resp.send(inner.await)
            .map_err(|_| anyhow::Error::msg("clear_all_caches failed to respond to message"))
    }

    async fn get_image_visibility(
        &self,
        resp: ESMResp<Visibility>,
        image: String,
    ) -> anyhow::Result<()> {
        let inner = async {
            // should we attempt to optimize the rwlock logic to batch writes?
            //
            // maybe batch with tokio::select! and recv_many -- have a separate writer
            // process with a 1024-length queue
            //
            // may not need timer, actually -- if we while let Some(_) = recv_many(...)
            // then we let things build up during each cycle

            // alternatively, prime the cache at boot time and periodically refresh it
            // in bulk, including after each scan
            //
            // then, after we commit changes to any images, refresh just that image

            Ok(Visibility::Private)
        };

        resp.send(inner.await)
            .map_err(|_| anyhow::Error::msg("clear_all_caches failed to respond to message"))
    }
}

#[async_trait]
impl ESInner for Caches {
    fn new() -> Self {
        Caches {
            image_visibility: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn message_handler(&self, esm: ESM) -> anyhow::Result<()> {
        match esm {
            ESM::Cache(message) => match message {
                CacheMsg::ClearAllCaches { resp } => self.clear_all_caches(resp).await,
                CacheMsg::GetImageVisibility { resp, image } => {
                    self.get_image_visibility(resp, image).await
                }
                _ => Err(anyhow::Error::msg("not implemented")),
            },
            _ => Err(anyhow::Error::msg("not implemented")),
        }
    }
}

pub struct CacheService {
    config: Arc<ESConfig>,
    sender: ESMSender,
    receiver: Arc<Mutex<ESMReceiver>>,
    handle: AsyncCell<tokio::task::JoinHandle<anyhow::Result<()>>>,
}

#[async_trait]
impl EntanglementService for CacheService {
    type Inner = Caches;

    fn create(config: Arc<ESConfig>) -> (ESMSender, Self) {
        let (tx, rx) = tokio::sync::mpsc::channel::<ESM>(32);

        (
            tx.clone(),
            CacheService {
                config: config.clone(),
                sender: tx,
                receiver: Arc::new(Mutex::new(rx)),
                handle: AsyncCell::new(),
            },
        )
    }

    async fn start(&self, senders: HashMap<ESM, ESMSender>) -> anyhow::Result<()> {
        // falliable stuff can happen here
        //
        // need to get senders for any cacheable data, starting with database service
        //
        // possibly want to also spin up the timer threads here

        let receiver = Arc::clone(&self.receiver);
        let state = Arc::new(Caches::new());

        let serve = {
            async move {
                while let Some(msg) = receiver.lock().await.recv().await {
                    let state = Arc::clone(&state);
                    tokio::task::spawn(async move {
                        match state.message_handler(msg).await {
                            Ok(()) => (),
                            Err(_) => println!("cache service failed to reply to message"),
                        }
                    });
                }

                Err::<(), anyhow::Error>(anyhow::Error::msg(format!("channel disconnected")))
            }
        };

        let handle = tokio::task::spawn(serve);

        self.handle.set(handle);

        Ok(())
    }
}
