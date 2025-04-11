use anyhow::Result;
use async_trait::async_trait;

use crate::service::ESInner;
use api::{
    library::LibraryUuid,
    task::{Task, TaskStatus, TaskType, TaskUid},
};

pub mod msg;
pub mod scan;
pub mod svc;
pub mod scan_utils;

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

// task debugging task
#[tracing::instrument]
async fn sleep_task(library_uuid: LibraryUuid) -> Result<i64> {
    tracing::info!("info from task");
    tokio::time::sleep(std::time::Duration::from_secs(100)).await;

    Ok(-1)
}
