use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use tokio::{sync::mpsc::Sender, task::JoinHandle};
use tracing::{error, instrument, Level};

use crate::{
    db::msg::DbMsg,
    service::{ESInner, ESMRegistry, ServiceType},
};
use api::{
    library::LibraryUuid,
    task::{LogLevel, Task, TaskLog, TaskType, TaskUid},
};

pub mod msg;
pub mod scan;
pub mod svc;

#[async_trait]
pub trait ESTaskService: ESInner {
    async fn start_task(
        &self,
        library_uuid: LibraryUuid,
        task_type: TaskType,
        uid: TaskUid,
    ) -> Result<LibraryUuid>;

    async fn stop_task(&self, library_uuid: LibraryUuid) -> Result<()>;

    // read the database for historical tasks and combine with current data
    async fn status(&self) -> Result<HashMap<LibraryUuid, Task>>;
}

// this isn't quite right -- we should probably have a trait ESTask and have the
// create() method take a log send fn
fn send_log(registry: ESMRegistry, log: TaskLog) {
    let db_svc_sender = match registry.get(&ServiceType::Db) {
        Ok(v) => v,
        Err(_) => {
            error!("send_log could not find db sender in registry");
            return;
        }
    };

    let logger = async move {
        let (tx, rx) = tokio::sync::oneshot::channel();
        db_svc_sender
            .send(
                DbMsg::AddLog {
                    resp: tx,
                    log: format!("{log:?}"),
                }
                .into(),
            )
            .await?;

        rx.await??;

        anyhow::Result::<()>::Ok(())
    };

    tokio::task::spawn(async {
        match logger.await {
            Ok(()) => {}
            Err(err) => {
                error!("failed to send log to database: {err}");
            }
        };
    });
}
