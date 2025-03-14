use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;

use crate::service::*;
use api::task::{Task, TaskUid, TaskUuid};

pub mod msg;
pub mod svc;
pub mod scan;

#[async_trait]
pub trait ESTaskService: ESInner {
    async fn start_task(&self, task: Task) -> Result<TaskUuid>;

    async fn stop_task(&self, task_uuid: TaskUuid, uid: TaskUid) -> Result<()>;

    async fn flush(&self) -> Result<()>;

    async fn status(&self) -> Result<HashMap<TaskUuid, Task>>;
}
