use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use mysql_async::{prelude::*, Pool};

use tokio::sync::Mutex;

use crate::db::msg::DbMsg;
use crate::service::{ESConfig, ESMReceiver, ESMResp, ESMSender, EntanglementService, ESM};

use super::ESDbService;

pub struct MySQLService {
    config: Arc<ESConfig>,
    sender: ESMSender,
    receiver: Mutex<ESMReceiver>,
}

#[async_trait]
impl ESDbService for MySQLService {
    async fn get_filtered_images(
        self: Arc<Self>,
        resp: ESMResp<()>,
        user: String,
        filter: String,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn edit_album(
        self: Arc<Self>,
        resp: ESMResp<()>,
        user: String,
        album: String,
        data: (),
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

#[async_trait]
impl EntanglementService for MySQLService {
    fn create(config: Arc<ESConfig>) -> (ESMSender, Arc<Self>) {
        let (tx, rx) = tokio::sync::mpsc::channel::<ESM>(32);

        (
            tx.clone(),
            Arc::new(MySQLService {
                config: config.clone(),
                sender: tx,
                receiver: Mutex::new(rx),
            }),
        )
    }

    async fn message_handler(self: Arc<Self>, esm: ESM) -> anyhow::Result<()> {
        match esm {
            ESM::Db(message) => match message {
                DbMsg::ImageListQuery { resp, user, filter } => {
                    self.get_filtered_images(resp, user, filter).await
                }
                DbMsg::EditAlbum { resp, user, album, data } => {
                    self.edit_album(resp, user, album, data).await
                }
                _ => Err(anyhow::Error::msg("not implemented")),
            },
            _ => Err(anyhow::Error::msg("not implemented")),
        }
    }

    async fn start(
        self: Arc<Self>,
        senders: HashMap<ESM, ESMSender>,
    ) -> anyhow::Result<tokio::task::JoinHandle<anyhow::Result<()>>> {

        let serve = {
            let self = Arc::clone(&self);
            async move {
                while let Some(msg) = self.clone().receiver.lock().await.recv().await {
                    let self = Arc::clone(&self);
                    tokio::task::spawn(async move {
                        match self.clone().message_handler(msg).await {
                            Ok(()) => (),
                            Err(err) => println!("mysql_service failed to reply to message"),
                        }
                    });
                }

                Err::<(), anyhow::Error>(anyhow::Error::msg(format!("channel disconnected")))
            }
        };
        Err(anyhow::Error::msg("not implemented"))
    }
}
