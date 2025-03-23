use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use async_cell::sync::AsyncCell;
use async_trait::async_trait;
use chrono::Local;
use dashmap::{DashMap, Entry};
use ringbuffer::AllocRingBuffer;
use tokio::{
    sync::{oneshot::channel, Mutex},
    task::{spawn, JoinHandle},
};
use tracing::{debug, error, info, instrument, warn, Level};

use crate::{
    db::msg::DbMsg,
    service::{
        ESInner, ESMReceiver, ESMRegistry, ESMSender, EntanglementService, ServiceType, ESM,
    },
    task::{msg::TaskMsg, scan::scan_library, ESTaskService},
};
use api::{
    library::LibraryUuid,
    task::{Task, TaskStatus, TaskType, TaskUid, TaskUuid},
};
use common::config::ESConfig;

// task service
//
// several of the common library operations (scan, clean, run scripts, etc) take too long
// for a single frontend api call.  instead, they are managed by this service, and send
// their logs directly to the database.
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
        info!("starting task service");

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

        debug!("started task service");
        Ok(())
    }
}

#[derive(Debug)]
pub struct TaskRunner {
    config: Arc<ESConfig>,
    registry: ESMRegistry,
    running_tasks: DashMap<LibraryUuid, AsyncCell<RunningTask>>,
    task_history: DashMap<LibraryUuid, AllocRingBuffer<Task>>,
}

#[derive(Clone, Debug)]
struct RunningTask {
    task: Task,
    handle: Arc<JoinHandle<()>>,
}

#[async_trait]
impl ESInner for TaskRunner {
    fn new(config: Arc<ESConfig>, registry: ESMRegistry) -> Result<Self> {
        Ok(TaskRunner {
            config: config.clone(),
            registry: registry.clone(),
            running_tasks: DashMap::new(),
            task_history: DashMap::new(),
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
                TaskMsg::StopTask { resp, library_uuid } => {
                    self.respond(resp, self.stop_task(library_uuid)).await
                }
                TaskMsg::Status { resp } => self.respond(resp, self.status()).await,
            },
            _ => Err(anyhow::Error::msg("not implemented")),
        }
    }
}

#[async_trait]
impl ESTaskService for TaskRunner {
    #[instrument(level=Level::DEBUG)]
    async fn start_task(
        &self,
        library_uuid: LibraryUuid,
        task_type: TaskType,
        uid: TaskUid,
    ) -> Result<()> {
        todo!()
    }

    #[instrument(level=Level::DEBUG)]
    async fn stop_task(&self, library_uuid: LibraryUuid) -> Result<()> {
        todo!()
    }

    async fn status(&self) -> Result<HashMap<LibraryUuid, Task>> {
        todo!()
    }
}
