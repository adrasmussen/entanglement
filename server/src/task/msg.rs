use crate::service::{Esm, EsmResp};
use api::task::*;

#[derive(Debug)]
pub enum TaskMsg {
    StartTask {
        resp: EsmResp<()>,
        library: TaskLibrary,
        task_type: TaskType,
        uid: TaskUid,
    },
    StopTask {
        resp: EsmResp<()>,
        library: TaskLibrary,
    },
    ShowTasks {
        resp: EsmResp<Vec<Task>>,
        library: TaskLibrary,
    },
    CompleteTask {
        resp: EsmResp<()>,
        library: TaskLibrary,
        status: TaskStatus,
        warnings: Option<i64>,
        end: u64,
    },
}

impl From<TaskMsg> for Esm {
    fn from(value: TaskMsg) -> Self {
        Esm::Task(value)
    }
}
