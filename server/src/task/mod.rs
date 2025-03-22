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
    task::{Task, TaskType, TaskUid},
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
