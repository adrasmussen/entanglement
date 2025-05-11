use anyhow::Result;
use async_trait::async_trait;

use crate::service::ESInner;
use api::{
    library::LibraryUuid,
    task::{Task, TaskStatus, TaskType, TaskUid},
};

pub mod clean;
pub mod msg;
pub mod scan;
pub mod scan_utils;
pub mod svc;

// pub mod dedup;
// pub mod dateparse;

#[async_trait]
pub trait ESTaskService: ESInner {
    async fn start_task(
        &self,
        library_uuid: LibraryUuid,
        task_type: TaskType,
        uid: TaskUid,
    ) -> Result<()>;

    async fn stop_task(&self, library_uuid: LibraryUuid) -> Result<()>;

    async fn show_tasks(&self, library_uuid: LibraryUuid) -> Result<Vec<Task>>;

    async fn complete_task(
        &self,
        library_uuid: LibraryUuid,
        status: TaskStatus,
        errors: Option<i64>,
        end: i64,
    ) -> Result<()>;
}
