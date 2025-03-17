use serde::{Deserialize, Serialize};

use crate::endpoint;
use crate::task::TaskUuid;

// structs and types

pub type LogUuid = i64;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TaskLog {
    pub task_uuid: TaskUuid,
    pub ctime: i64,
    pub level: LogLevel,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}
