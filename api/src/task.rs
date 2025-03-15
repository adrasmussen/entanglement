use serde::{Deserialize, Serialize};

use crate::library::LibraryUuid;

// structs and types
pub type TaskUuid = i64;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TaskType {
    ScanLibrary,
    CleanLibrary,
    RunScripts,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Unknown,
    Running,
    Success,
    Failure,
    Orphaned,
    Aborted,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TaskUid {
    User { uid: String },
    System,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Task {
    pub library_uuid: LibraryUuid,
    pub task_type: TaskType,
    pub uid: TaskUid,
    pub status: TaskStatus,
    pub start: i64,
    pub end: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TaskUpdate {
    pub status: Option<TaskStatus>,
    pub end: Option<i64>,
}
