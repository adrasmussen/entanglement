use serde::{Deserialize, Serialize};

use crate::endpoint;
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

// messages

// start a task on a library
endpoint!(StartTask);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StartTaskReq {
    pub library_uuid: LibraryUuid,
    pub task_type: TaskType,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StartTaskResp {
    pub task_uuid: TaskUuid,
}

// stop a running task
endpoint!(StopTask);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StopTaskReq {
    pub library_uuid: LibraryUuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StopTaskResp {}

// get a task
endpoint!(GetTask);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetTaskReq {
    pub task_uuid: TaskUuid
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetTaskResp {
    pub task: Task,
}

// search tasks
endpoint!(SearchTasks);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchTasksReq {
    pub filter: Option<TaskStatus>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchTasksResp {
    pub tasks: Vec<TaskUuid>,
}
