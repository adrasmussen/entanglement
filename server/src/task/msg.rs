use crate::service::{EsmResp, Esm};
use api::{library::LibraryUuid, task::*};

#[derive(Debug)]
pub enum TaskMsg {
    StartTask {
        resp: EsmResp<()>,
        library_uuid: LibraryUuid,
        task_type: TaskType,
        uid: TaskUid,
    },
    StopTask {
        resp: EsmResp<()>,
        library_uuid: LibraryUuid,
    },
    ShowTasks {
        resp: EsmResp<Vec<Task>>,
        library_uuid: LibraryUuid,
    },
    CompleteTask {
        resp: EsmResp<()>,
        library_uuid: LibraryUuid,
        status: TaskStatus,
        warnings: Option<i64>,
        end: i64,
    },
}

impl From<TaskMsg> for Esm {
    fn from(value: TaskMsg) -> Self {
        Esm::Task(value)
    }
}
