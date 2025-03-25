use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use tracing::error;

use crate::{
    db::msg::DbMsg,
    service::{ESInner, ESMSender},
};
use api::{
    library::LibraryUuid,
    task::{Task, TaskStatus, TaskType, TaskUid},
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
    ) -> Result<()>;

    async fn stop_task(&self, library_uuid: LibraryUuid) -> Result<()>;

    // read the database for historical tasks and combine with current data
    async fn status(&self, library_uuid: LibraryUuid) -> Result<Vec<Task>>;

    // message from spawned tasks when the watcher future completes or is aborted
    //
    // must be Result<()> because there is no responder
    async fn complete_task(&self, library_uuid: LibraryUuid, status: TaskStatus, errors: Option<i64>, end: i64) -> Result<()>;
}
