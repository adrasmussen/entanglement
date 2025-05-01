use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::{endpoint, library::LibraryUuid};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum TaskType {
    ScanLibrary,
    CleanLibrary,
    RunScripts,
    //VerifyMime,
    //AsyncTranscode,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum TaskStatus {
    Unknown,
    Running,
    Success,
    Failure,
    Aborted,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum TaskUid {
    User { uid: String },
    System,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Task {
    pub task_type: TaskType,
    pub uid: TaskUid,
    pub status: TaskStatus,
    pub warnings: Option<i64>,
    pub start: i64,
    pub end: Option<i64>,
}

// messages

// start a task on a library
endpoint!(StartTask);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StartTaskReq {
    pub library_uuid: LibraryUuid,
    pub task_type: TaskType,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StartTaskResp {}

// stop a running task
endpoint!(StopTask);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StopTaskReq {
    pub library_uuid: LibraryUuid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StopTaskResp {}

// show tasks
endpoint!(ShowTasks);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ShowTasksReq {
    pub library_uuid: LibraryUuid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ShowTasksResp {
    pub tasks: Vec<Task>,
}

// display impls so that we can output these cleanly to logs
impl Display for TaskType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Display for TaskUid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::User { uid } => write!(f, "{uid}"),
            Self::System => write!(f, "system"),
        }
    }
}
