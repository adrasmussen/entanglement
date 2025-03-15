use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;

use crate::service::*;
use api::{library::LibraryUuid, task::{Task, TaskType, TaskUid, TaskUuid}};

pub mod msg;
pub mod svc;
pub mod scan;

#[async_trait]
pub trait ESTaskService: ESInner {
    async fn start_task(&self, library_uuid: LibraryUuid, task_type: TaskType, uid: TaskUid) -> Result<TaskUuid>;

    async fn stop_task(&self, task_uuid: TaskUuid, uid: TaskUid) -> Result<()>;

    // read the database for historical tasks and combine with current data
    async fn status(&self) -> Result<HashMap<TaskUuid, Task>>;
}
