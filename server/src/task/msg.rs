use crate::service::{ESMResp, ESM};
use api::{library::LibraryUuid, task::*};

#[derive(Debug)]
pub enum TaskMsg {
    StartTask {
        resp: ESMResp<()>,
        library_uuid: LibraryUuid,
        task_type: TaskType,
        uid: TaskUid,
    },
    StopTask {
        resp: ESMResp<()>,
        library_uuid: LibraryUuid,
    },
    ShowTasks {
        resp: ESMResp<Vec<Task>>,
        library_uuid: LibraryUuid,
    },
    CompleteTask {
        resp: ESMResp<()>,
        library_uuid: LibraryUuid,
        status: TaskStatus,
        warnings: Option<i64>,
        end: i64,
    },
}

impl From<TaskMsg> for ESM {
    fn from(value: TaskMsg) -> Self {
        ESM::Task(value)
    }
}
