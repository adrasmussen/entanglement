use std::fmt::Display;

use anyhow::Result;
use async_trait::async_trait;

use crate::service::ESInner;
use api::{
    library::LibraryUuid,
    task::{Task, TaskStatus, TaskType, TaskUid},
};

mod clean;
pub mod msg;
mod scan;
mod scan_utils;
mod scrub;
pub mod svc;

// pub mod dedup;
// pub mod dateparse;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TaskLibrary {
    User { library_uuid: LibraryUuid },
    System,
}

impl Display for TaskLibrary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
            Self::User { library_uuid } => write!(f, "{library_uuid}"),
            Self::System => write!(f, "system"),
        }
    }
}

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
