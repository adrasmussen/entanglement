use std::collections::HashMap;
use std::sync::Arc;

use api::AlbumMetadata;
use async_cell::sync::AsyncCell;

use async_trait::async_trait;

use mysql_async::{prelude::*, Pool};

use tokio::sync::Mutex;

use crate::db::msg::DbMsg;
use crate::service::{ESConfig, ESMReceiver, ESMResp, ESMSender, EntanglementService, ESM, ESInner};

use super::ESDbService;



pub struct MySQLState {
    //conn: mysql_async::Conn,
}

#[async_trait]
impl ESDbService for MySQLState {
    async fn get_filtered_images(
        &self,
        resp: ESMResp<()>,
        user: String,
        filter: String,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn edit_album(
        &self,
        resp: ESMResp<()>,
        user: String,
        album: String,
        change: AlbumMetadata,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

#[async_trait]
impl ESInner for MySQLState {
    fn new () -> Self {
        MySQLState {}
    }

    async fn message_handler(&self, esm: ESM) -> anyhow::Result<()> {
        match esm {
            ESM::Db(message) => match message {
                DbMsg::ImageListQuery { resp, user, filter } => {
                    self.get_filtered_images(resp, user, filter).await
                }
                DbMsg::UpdateAlbum {
                    resp,
                    user,
                    album,
                    change,
                } => self.edit_album(resp, user, album, change).await,
                _ => Err(anyhow::Error::msg("not implemented")),
            },
            _ => Err(anyhow::Error::msg("not implemented")),
        }
    }
}

pub struct MySQLService {
    config: Arc<ESConfig>,
    sender: ESMSender,
    receiver: Arc<Mutex<ESMReceiver>>,
    handle: AsyncCell<tokio::task::JoinHandle<anyhow::Result<()>>>,
}

#[async_trait]
impl EntanglementService for MySQLService {
    type Inner = MySQLState;

    fn create(config: Arc<ESConfig>) -> (ESMSender, Self) {
        let (tx, rx) = tokio::sync::mpsc::channel::<ESM>(32);

        (
            tx.clone(),
            MySQLService {
                config: config.clone(),
                sender: tx,
                receiver: Arc::new(Mutex::new(rx)),
                handle: AsyncCell::new(),
            },
        )
    }

    async fn start(&self, senders: HashMap<ESM, ESMSender>) -> anyhow::Result<()> {
        // falliable stuff can happen here

        let receiver = Arc::clone(&self.receiver);
        let state = Arc::new(MySQLState {});

        let serve = {
            async move {
                while let Some(msg) = receiver.lock().await.recv().await {
                    let state = Arc::clone(&state);
                    tokio::task::spawn(async move {
                        match state.message_handler(msg).await {
                            Ok(()) => (),
                            Err(_) => println!("mysql_service failed to reply to message"),
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
