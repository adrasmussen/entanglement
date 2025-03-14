use std::collections::HashMap;

use crate::service::{ESMResp, ESM};
use api::task::*;

#[derive(Debug)]
pub enum TaskMsg {
    StartTask {
        resp: ESMResp<TaskUuid>,
        task: Task,
    },
    StopTask {
        resp: ESMResp<()>,
        task_uuid: TaskUuid,
        uid: TaskUid,
    },
    Flush {
        resp: ESMResp<()>,
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
