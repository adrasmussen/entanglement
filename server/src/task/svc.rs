use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use anyhow::Result;
use async_cell::sync::AsyncCell;
use async_trait::async_trait;
use chrono::Local;
use dashmap::{DashMap, Entry};
use tokio::{
    sync::{oneshot::channel, Mutex},
    task::{spawn, JoinHandle},
};
use tracing::{debug, error, info, instrument, warn, Level};

use crate::service::{
    ESInner, ESMReceiver, ESMRegistry, ESMSender, EntanglementService, ServiceType, ESM,
};
use crate::task::{msg::TaskMsg, scan::scan_library, ESTaskService};
use crate::{auth::check::AuthCheck, db::msg::DbMsg};
use api::{library::LibraryUuid, task::*};
use common::config::ESConfig;

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

        registry
            .insert(ServiceType::Task, tx)
            .expect("failed to add task sender to registry");

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
    running_tasks: DashMap<LibraryUuid, Option<RunningTask>>,
}

#[derive(Clone, Debug)]
struct RunningTask {
    task: Task,
    uuid: TaskUuid,
    handle: Arc<JoinHandle<()>>,
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
                TaskMsg::StartTask {
                    resp,
                    library_uuid,
                    task_type,
                    uid,
                } => {
                    self.respond(resp, self.start_task(library_uuid, task_type, uid))
                        .await
                }
                TaskMsg::StopTask {
                    resp,
                    task_uuid,
                    uid,
                } => self.respond(resp, self.stop_task(task_uuid, uid)).await,
                TaskMsg::Status { resp } => self.respond(resp, self.status()).await,
            },
            _ => Err(anyhow::Error::msg("not implemented")),
        }
    }
}

impl AuthCheck for TaskRunner {}

#[instrument(level=Level::DEBUG, skip(task_future, db_svc_sender))]
fn spawn_task<F>(task_uuid: TaskUuid, task_future: F, db_svc_sender: ESMSender) -> JoinHandle<()>
where
    F: Future<Output = Result<()>> + Send + 'static,
{
    let db_svc_sender = db_svc_sender.clone();

    spawn(async move {
        let res = task_future.await;

        let (tx, rx) = channel();

        let status = match res {
            Ok(()) => {
                info!("task succeeded");
                TaskStatus::Success
            }
            Err(_) => {
                warn!("task failed");
                TaskStatus::Failure
            }
        };

        match db_svc_sender
            .send(
                DbMsg::UpdateTask {
                    resp: tx,
                    task_uuid: task_uuid,
                    update: TaskUpdate {
                        status: Some(status),
                        end: Some(Local::now().timestamp()),
                    },
                }
                .into(),
            )
            .await
        {
            Ok(_) => {}
            Err(err) => {
                error!("failed to send UpdateTask message to db service: {err}");
            }
        };

        match rx.await {
            Ok(Ok(_)) => {}
            Ok(Err(err)) => {
                error!("failed to update task in database: {err}");
            }
            Err(err) => {
                error!("failed to receive UpdateTask response from db service: {err}")
            }
        };
    })
}

#[async_trait]
impl ESTaskService for TaskRunner {
    #[instrument(level=Level::DEBUG)]
    async fn start_task(
        &self,
        library_uuid: LibraryUuid,
        task_type: TaskType,
        uid: TaskUid,
    ) -> Result<TaskUuid> {
        debug!("task received by runner");

        let db_svc_sender = self.registry.get(&ServiceType::Db)?;

        match self.running_tasks.entry(library_uuid) {
            Entry::Occupied(_) => {
                return Err(anyhow::Error::msg(format!(
                    "cannot start {task_type:?} -- a task is already running for library {library_uuid}"
                )))
            }
            Entry::Vacant(entry) => {
                entry.insert(None);
            }
        }

        let dispatch = async {
            let (library_tx, library_rx) = channel();

            db_svc_sender
                .send(
                    DbMsg::GetLibrary {
                        resp: library_tx,
                        library_uuid: library_uuid,
                    }
                    .into(),
                )
                .await?;

            let library = library_rx.await??.ok_or_else(|| {
                anyhow::Error::msg(format!("unknown library uuid {}", library_uuid))
            })?;

            let (task_tx, task_rx) = channel();

            // repackage the task to overwrite the dynamic parts
            let task = Task {
                library_uuid: library_uuid,
                task_type: task_type,
                uid: uid,
                status: TaskStatus::Running,
                start: Local::now().timestamp(),
                end: None,
            };

            // even if the task fails immediately (or we don't even get the uuid back),
            // we want to record the attempt
            db_svc_sender
                .send(
                    DbMsg::AddTask {
                        resp: task_tx,
                        task: task.clone(),
                    }
                    .into(),
                )
                .await?;

            let task_uuid = task_rx.await??;

            debug!("task recorded in database");

            let handle = match task.task_type {
                TaskType::ScanLibrary => spawn_task(task_uuid, scan_library(), db_svc_sender),
                _ => {
                    error!({ task_uuid = task_uuid, task_type = ?task.task_type }, "unknown task type");
                    return Err(anyhow::Error::msg("unknown task type"));
                }
            };

            debug!("task dispatched");

            Ok(RunningTask {
                task: task,
                uuid: task_uuid,
                handle: Arc::new(handle),
            })
        };

        match dispatch.await {
            Ok(rt) => {
                let task_uuid = rt.uuid;
                self.running_tasks.alter(&library_uuid, |_, _| Some(rt));
                Ok(task_uuid)
            }
            Err(err) => {
                error!("failed to dispatch task: {err}");
                self.running_tasks.remove(&library_uuid);
                Err(anyhow::Error::msg(format!(
                    "failed to dispatch task: {err}"
                )))
            }
        }
    }

    async fn stop_task(&self, task_uuid: TaskUuid, uid: TaskUid) -> Result<()> {
        todo!()
    }

    async fn status(&self) -> Result<HashMap<TaskUuid, Task>> {
        todo!()
    }
}
