use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::{http_endpoint, library::LibraryUuid};

// structs and types
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum TaskLibrary {
    User { library_uuid: LibraryUuid },
    System,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum TaskType {
    ScanLibrary,
    CleanLibrary,
    RunScripts,
    CacheScrub,
    //VerifyMime,
    //AsyncTranscode,
    //RecalculateHashes,
    //GuessDate
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
    pub start: u64,
    pub end: Option<u64>,
}

// messages

// start a task on a library
http_endpoint!(StartTask);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StartTaskReq {
    pub library_uuid: LibraryUuid,
    pub task_type: TaskType,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StartTaskResp {}

// stop a running task
http_endpoint!(StopTask);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StopTaskReq {
    pub library_uuid: LibraryUuid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StopTaskResp {}

// show tasks
http_endpoint!(ShowTasks);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ShowTasksReq {
    pub library: TaskLibrary,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ShowTasksResp {
    pub tasks: Vec<Task>,
}

// display impls so that we can output these cleanly to logs
impl Display for TaskLibrary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::User { library_uuid } => write!(f, "{library_uuid}"),
            Self::System => write!(f, "system"),
        }
    }
}

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
