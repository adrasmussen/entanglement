use anyhow::Result;
use async_trait::async_trait;

use crate::service::ESInner;
use api::task::{Task, TaskLibrary, TaskStatus, TaskType, TaskUid};

mod clean;
pub mod msg;
mod scan;
mod scan_utils;
mod scrub;
pub mod svc;

// pub mod dedup;
// pub mod dateparse;

#[async_trait]
pub trait ESTaskService: ESInner {
    async fn start_task(
        &self,
        library: TaskLibrary,
        task_type: TaskType,
        uid: TaskUid,
    ) -> Result<()>;

    async fn stop_task(&self, library: TaskLibrary) -> Result<()>;

    async fn show_tasks(&self, library: TaskLibrary) -> Result<Vec<Task>>;

    async fn complete_task(
        &self,
        library: TaskLibrary,
        status: TaskStatus,
        errors: Option<i64>,
        end: i64,
    ) -> Result<()>;
}
