use std::collections::HashMap;

use crate::service::{ESMResp, ESM};
use api::{library::LibraryUuid, task::*};

#[derive(Debug)]
pub enum TaskMsg {
    StartTask {
        resp: ESMResp<TaskUuid>,
        library_uuid: LibraryUuid,
        task_type: TaskType,
        uid: TaskUid
    },
    StopTask {
        resp: ESMResp<()>,
        task_uuid: TaskUuid,
        uid: TaskUid,
    },
    Status {
        resp: ESMResp<HashMap<TaskUuid, Task>>,
    },
}

impl From<TaskMsg> for ESM {
    fn from(value: TaskMsg) -> Self {
        ESM::Task(value)
    }
}
