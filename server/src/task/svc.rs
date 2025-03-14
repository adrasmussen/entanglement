use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use async_cell::sync::AsyncCell;
use async_trait::async_trait;
use chrono::Local;
use dashmap::DashMap;
use tokio::{
    sync::{oneshot::channel, Mutex},
    task::{spawn, JoinHandle},
};
use tracing::{debug, error, info, instrument, warn, Level};

use crate::db::msg::DbMsg;
use crate::service::{
    ESInner, ESMReceiver, ESMRegistry, ESMSender, EntanglementService, ServiceType, ESM,
};
use crate::task::{msg::TaskMsg, scan::scan_library, ESTaskService};
use api::{library::LibraryUuid, task::*};
use common::config::ESConfig;

const ADMIN_TASKS: [TaskType; 0] = [];

pub struct TaskService {
    config: Arc<ESConfig>,
    receiver: Arc<Mutex<ESMReceiver>>,
    handle: AsyncCell<tokio::task::JoinHandle<Result<()>>>,
}

#[async_trait]
impl EntanglementService for TaskService {
    type Inner = TaskRunner;

    fn create(config: Arc<ESConfig>, registry: &ESMRegistry) -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel::<ESM>(1024);

        registry.insert(ServiceType::Task, tx);

        TaskService {
            config: config.clone(),
            receiver: Arc::new(Mutex::new(rx)),
            handle: AsyncCell::new(),
        }
    }

    #[instrument(level=Level::DEBUG, skip(self, registry))]
    async fn start(&self, registry: &ESMRegistry) -> Result<()> {
        info!("starting");

        let receiver = Arc::clone(&self.receiver);
        let state = Arc::new(TaskRunner::new(self.config.clone(), registry.clone())?);

        let serve = {
            async move {
                let mut receiver = receiver.lock().await;

                while let Some(msg) = receiver.recv().await {
                    let state = Arc::clone(&state);
                    tokio::task::spawn(async move {
                        match state.message_handler(msg).await {
                            Ok(()) => (),
                            Err(err) => {
                                error!({service = "task", channel = "esm", error = %err})
                            }
                        }
                    });
                }

                Err(anyhow::Error::msg(format!(
                    "task service esm channel disconnected"
                )))
            }
        };

        self.handle.set(tokio::task::spawn(serve));

        debug!("started");
        Ok(())
    }
}

#[derive(Debug)]
pub struct TaskRunner {
    config: Arc<ESConfig>,
    registry: ESMRegistry,
    db_svc_sender: ESMSender,
    running_tasks: DashMap<LibraryUuid, RunningTask>,
}

#[derive(Debug)]
struct RunningTask {
    task: Task,
    uuid: TaskUuid,
    handle: JoinHandle<Result<()>>,
}

#[async_trait]
impl ESInner for TaskRunner {
    fn new(config: Arc<ESConfig>, registry: ESMRegistry) -> Result<Self> {
        Ok(TaskRunner {
            config: config.clone(),
            registry: registry.clone(),
            db_svc_sender: registry
                .get(&ServiceType::Db)
                .expect("task service failed to find db service sender")
                .clone(),
            running_tasks: DashMap::new(),
        })
    }

    fn registry(&self) -> ESMRegistry {
        self.registry.clone()
    }

    async fn message_handler(&self, esm: ESM) -> Result<()> {
        match esm {
            ESM::Task(message) => match message {
                TaskMsg::StartTask { resp, task } => {
                    self.respond(resp, self.start_task(task)).await
                }
                TaskMsg::StopTask {
                    resp,
                    task_uuid,
                    uid,
                } => self.respond(resp, self.stop_task(task_uuid, uid)).await,
                TaskMsg::Flush { resp } => self.respond(resp, self.flush()).await,
                TaskMsg::Status { resp } => self.respond(resp, self.status()).await,
            },
            _ => Err(anyhow::Error::msg("not implemented")),
        }
    }
}

#[async_trait]
impl ESTaskService for TaskRunner {
    #[instrument(level=Level::DEBUG)]
    async fn start_task(&self, task: Task) -> Result<TaskUuid> {
        debug!("task received by runner");

        let library_uuid = task.library_uuid;

        // sanity checks
        if self.running_tasks.contains_key(&library_uuid) {
            warn!(
                { library_uuid = library_uuid },
                "library is already running a task"
            );
            return Err(anyhow::Error::msg(format!(
                "library {} is already running a task",
                library_uuid
            )));
        }

        if task.uid != TaskUid::System && ADMIN_TASKS.contains(&task.task_type) {
            warn!({uid = ?task.uid, task_type = ?task.task_type, library_uuid = library_uuid}, "user attempted to run admin-only task");
            return Err(anyhow::Error::msg(
                "non-system users cannot run administrative tasks",
            ));
        }

        let (library_tx, library_rx) = channel();

        self.db_svc_sender
            .send(
                DbMsg::GetLibrary {
                    resp: library_tx,
                    library_uuid: library_uuid,
                }
                .into(),
            )
            .await?;

        let library = library_rx
            .await??
            .ok_or_else(|| anyhow::Error::msg(format!("unknown library uuid {}", library_uuid)))?;

        let (task_tx, task_rx) = channel();

        // repackage the task to overwrite the dynamic parts
        let task = Task {
            library_uuid: task.library_uuid,
            task_type: task.task_type,
            uid: task.uid,
            status: TaskStatus::Running,
            start: Local::now().timestamp(),
            end: None,
        };

        // even if the task fails immediately (or we don't even get the uuid back),
        // we want to record the attempt
        self.db_svc_sender
            .send(
                TaskMsg::StartTask {
                    resp: task_tx,
                    task: task.clone(),
                }
                .into(),
            )
            .await?;

        let task_uuid = task_rx.await??;

        debug!({ task_uuid = task_uuid }, "task recorded in database");

        let handle = match task.task_type {
            TaskType::ScanLibrary => {
                spawn(scan_library());
            }
            _ => {
                error!({ task_uuid = task_uuid, task_type = ?task.task_type }, "unknown task type");
                return Err(anyhow::Error::msg("unknown task type"));
            }
        };

        debug!({ task_uuid = task_uuid }, "task dispatched");
        Ok(task_uuid)
    }

    async fn stop_task(&self, task_uuid: TaskUuid, uid: TaskUid) -> Result<()> {
        todo!()
    }

    async fn flush(&self) -> Result<()> {
        todo!()
    }

    async fn status(&self) -> Result<HashMap<TaskUuid, Task>> {
        todo!()
    }
}
