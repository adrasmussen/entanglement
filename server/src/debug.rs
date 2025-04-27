use std::{sync::Arc, time::Duration};

use anyhow::Result;
use async_cell::sync::AsyncCell;
use async_trait::async_trait;
use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};
use tokio::{sync::Mutex, task::JoinHandle, time::sleep};
use tracing::{debug, error, info, instrument};

use crate::{
    http::auth::CurrentUser,
    service::{ESInner, ESMReceiver, ESMRegistry, EntanglementService, ServiceType, ESM},
};
use api::library::LibraryUuid;
use common::config::ESConfig;

// debugging tools
//
// this module contains several constructs that are useful for debugging the server,
// but are not meant to be used in production as they may caused defined but almost
// certainly unintended behavior.

// task debugging task
#[allow(dead_code)]
#[instrument]
pub async fn sleep_task(library_uuid: LibraryUuid) -> Result<i64> {
    info!("info from task");
    sleep(Duration::from_secs(100)).await;

    Ok(-1)
}

// auth bypass middleware
#[allow(dead_code)]
pub async fn bypass_auth(mut req: Request, next: Next) -> Result<Response, StatusCode> {
    req.extensions_mut().insert(CurrentUser {
        uid: String::from("alex"),
    });

    Ok(next.run(req).await)
}

// echo service to print messages
#[allow(dead_code)]
pub struct EchoService {
    config: Arc<ESConfig>,
    receiver: Arc<Mutex<ESMReceiver>>,
    msg_handle: AsyncCell<JoinHandle<Result<()>>>,
}

#[async_trait]
impl EntanglementService for EchoService {
    type Inner = EchoInner;

    fn create(config: Arc<ESConfig>, registry: &ESMRegistry) -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel::<ESM>(1024);

        registry
            .insert(ServiceType::Echo, tx)
            .expect("failed to add echo sender to registry");

        EchoService {
            config: config.clone(),
            receiver: Arc::new(Mutex::new(rx)),
            msg_handle: AsyncCell::new(),
        }
    }

    #[instrument(skip(self, registry))]
    async fn start(&self, registry: &ESMRegistry) -> Result<()> {
        let receiver = Arc::clone(&self.receiver);
        let state = Arc::new(EchoInner::new(self.config.clone(), registry.clone())?);

        let msg_serve = {
            async move {
                let mut receiver = receiver.lock().await;

                while let Some(msg) = receiver.recv().await {
                    let state = Arc::clone(&state);
                    tokio::task::spawn(async move {
                        match state.message_handler(msg).await {
                            Ok(()) => (),
                            Err(err) => {
                                error!({service = "echo", channel = "esm", error = %err})
                            }
                        }
                    });
                }

                Err(anyhow::Error::msg(format!(
                    "http_service esm channel disconnected"
                )))
            }
        };

        let msg_handle = tokio::task::spawn(msg_serve);
        self.msg_handle.set(msg_handle);

        debug!("started http service");
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct EchoInner {
    registry: ESMRegistry,
}

#[async_trait]
impl ESInner for EchoInner {
    fn new(_config: Arc<ESConfig>, registry: ESMRegistry) -> Result<Self> {
        Ok(EchoInner { registry })
    }

    fn registry(&self) -> ESMRegistry {
        self.registry.clone()
    }

    #[instrument(skip_all)]
    async fn message_handler(&self, esm: ESM) -> Result<()> {
        info!("message: {esm:#?}");
        Ok(())
    }
}
