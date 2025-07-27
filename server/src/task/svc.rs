use std::{pin::Pin, sync::Arc};

use anyhow::Result;
use async_cell::sync::AsyncCell;
use async_trait::async_trait;
use dashmap::{DashMap, Entry};
use futures::Future;
use ringbuffer::{AllocRingBuffer, RingBuffer};
use tokio::{
    select,
    sync::{Mutex, RwLock},
    task::{JoinHandle, spawn},
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, instrument};

use crate::{
    db::msg::DbMsg,
    debug::sleep_task,
    service::{
        ESInner, ESMRegistry, EntanglementService, Esm, EsmReceiver, EsmSender, ServiceType,
    },
    task::{
        ESTaskService, clean::clean_library, msg::TaskMsg, scan::scan_library, scrub::cache_scrub,
    },
};
use api::task::{Task, TaskLibrary, TaskStatus, TaskType, TaskUid};
use common::{config::ESConfig, unix_time};

// task service
//
// several of the common library operations (scan, clean, run scripts, etc) take too long
// for a single frontend api call.  instead, they are managed by this service.
pub struct TaskService {
    config: Arc<ESConfig>,
    receiver: Arc<Mutex<EsmReceiver>>,
    handle: AsyncCell<tokio::task::JoinHandle<Result<()>>>,
}

#[async_trait]
impl EntanglementService for TaskService {
    type Inner = TaskRunner;

    fn create(config: Arc<ESConfig>, registry: &ESMRegistry) -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel::<Esm>(1024);

        registry
            .insert(ServiceType::Task, tx)
            .expect("failed to add task sender to registry");

        TaskService {
            config: config.clone(),
            receiver: Arc::new(Mutex::new(rx)),
            handle: AsyncCell::new(),
        }
    }

    #[instrument(skip(self, registry))]
    async fn start(&self, registry: &ESMRegistry) -> Result<()> {
        info!("starting task service");

        let receiver = Arc::clone(&self.receiver);
        let state = Arc::new(TaskRunner::new(self.config.clone(), registry.clone())?);

        let serve = {
            async move {
                let mut receiver = receiver.lock().await;

                while let Some(msg) = receiver.recv().await {
                    let state = Arc::clone(&state);
                    spawn(async move {
                        match state.message_handler(msg).await {
                            Ok(()) => (),
                            Err(err) => {
                                error!({service = "task", channel = "esm", error = %err})
                            }
                        }
                    });
                }

                Err(anyhow::Error::msg("task service esm channel disconnected"))
            }
        };

        self.handle.set(spawn(serve));

        debug!("started task service");
        Ok(())
    }
}

#[derive(Debug)]
pub struct TaskRunner {
    config: Arc<ESConfig>,
    registry: ESMRegistry,
    // has an extra layer of abstraction (Option<_>) so that we can hold the lock
    // until the task successfully starts without blocking the DashMap
    running_tasks: DashMap<TaskLibrary, Arc<RwLock<Option<RunningTask>>>>,
    task_history: DashMap<TaskLibrary, Arc<RwLock<AllocRingBuffer<Task>>>>,
}

#[derive(Debug)]
struct RunningTask {
    task: Task,
    cancel: CancellationToken,
    _handle: JoinHandle<()>,
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

    async fn message_handler(&self, esm: Esm) -> Result<()> {
        match esm {
            Esm::Task(message) => match message {
                TaskMsg::StartTask {
                    resp,
                    library,
                    task_type,
                    uid,
                } => {
                    self.respond(resp, self.start_task(library, task_type, uid))
                        .await
                }
                TaskMsg::StopTask { resp, library } => {
                    self.respond(resp, self.stop_task(library)).await
                }
                TaskMsg::ShowTasks { resp, library } => {
                    self.respond(resp, self.show_tasks(library)).await
                }
                TaskMsg::CompleteTask {
                    resp,
                    library,
                    status,
                    warnings,
                    end,
                } => {
                    self.respond(resp, self.complete_task(library, status, warnings, end))
                        .await
                }
            },
            _ => Err(anyhow::Error::msg("not implemented")),
        }
    }
}

// task runner
//
// there is a highly nontrivial interlock between start_task() and complete_task().
//
// specifically, start_task() will put a task into the running_task Option, provided
// that none exists, holding the lock until that task has been successfully launched
// into the executor.  it saves the task metadata and cancel channel sender.
//
// then complete_task() will remove the task, provided that it exists, and push it
// to the head of the history ring buffer.  it holds the lock just long enough to
// call take() on the Option.
//
// when inserting into either DashMap, we first check if the key is populated, and
// create it if not.
#[async_trait]
impl ESTaskService for TaskRunner {
    #[instrument(skip(self))]
    async fn start_task(
        &self,
        library: TaskLibrary,
        task_type: TaskType,
        uid: TaskUid,
    ) -> Result<()> {
        debug!("task pre-startup verification");
        let db_svc_sender = self.registry().get(&ServiceType::Db)?;
        let task_svc_sender = self.registry.get(&ServiceType::Task)?;

        // library verification
        if let TaskLibrary::User { library_uuid } = library {
            let (db_tx, db_rx) = tokio::sync::oneshot::channel();

            db_svc_sender
                .send(
                    DbMsg::GetLibrary {
                        resp: db_tx,
                        library_uuid,
                    }
                    .into(),
                )
                .await?;

            db_rx.await??;
        }

        // create the library's entry in the running task map if it doesn't exist
        //
        // this should be the only place that entries are put into the running DashMap,
        // all other calls should error somehow
        let rt_entry = match self.running_tasks.entry(library) {
            Entry::Occupied(entry) => entry.get().clone(),
            Entry::Vacant(entry) => {
                let v = Arc::new(RwLock::new(None));
                entry.insert(v.clone());
                v
            }
        };

        // grab the lock for the entirety of the startup action
        //
        // since this locks independently of the history and we only want one
        // thread attempting to start a task, this is reasonable
        let mut running_task = rt_entry.write().await;

        if running_task.is_some() {
            return Err(anyhow::Error::msg("task already running"));
        }

        // task futures
        //
        // each task is a Future<Output=Result<T>> that is spawned into the executor by a separate
        // "watcher" future which uses the select! macro to await either the future or a cancel token.
        //
        // currently, T = i64, the number of non-fatal errors encountered by the task.  in the future,
        // it could be a Box<dyn TaskReport> or similar
        //
        // what "failed" means depends on the task -- since tasks should typically produce reasonable
        // tracing logs, failure could either be catastrophic failure or a single error
        let start = unix_time();
        let config = self.config.clone();
        let registry = self.registry.clone();

        let task_future: Pin<Box<dyn Future<Output = Result<i64>> + Send>> = match library {
            // user library tasks
            TaskLibrary::User { library_uuid } => match task_type {
                TaskType::ScanLibrary => Box::pin(scan_library(config, registry, library_uuid)),
                TaskType::CleanLibrary => Box::pin(clean_library(config, registry, library_uuid)),
                TaskType::RunScripts => Box::pin(sleep_task(library_uuid)),
                _ => return Err(anyhow::Error::msg("unsupported user task")),
            },

            // system-wide tasks
            TaskLibrary::System => match task_type {
                TaskType::CacheScrub => Box::pin(cache_scrub(config, registry)),
                _ => return Err(anyhow::Error::msg("unsupported system task")),
            },
        };

        info!({ start = start }, "starting task");

        // spawn the future inside of the infalliable watcher and send the result back via ESM
        let (_handle, cancel) = watch_task(start, library, task_svc_sender, task_future);

        // we still have the write lock on the currently running task, but in principle the task
        // may have already completed and sent the message to complete_task().  thus, even if we
        // immediately take() the running task, it was still TaskStatus::Running for a short time.
        let task = Task {
            task_type: task_type.clone(),
            uid,
            status: TaskStatus::Running,
            warnings: None,
            start,
            end: None,
        };

        *running_task = Some(RunningTask {
            task,
            cancel,
            _handle,
        });

        Ok(())
    }

    #[instrument(skip(self))]
    async fn stop_task(&self, library: TaskLibrary) -> Result<()> {
        let rt_entry = self
            .running_tasks
            .get(&library)
            .ok_or_else(|| anyhow::Error::msg(format!("library {library} has no running task")))?;

        // even if we send a stop task message immediately after starting a task, we will wait
        // until it is successfully running and set into the dashmap before freeing the lock
        let running_task = rt_entry.read().await;

        let running_task = match running_task.as_ref() {
            Some(task) => task,
            None => {
                return Err(anyhow::Error::msg(format!(
                    "library {library} has no running task"
                )));
            }
        };

        info!({ start = running_task.task.start }, "stopping task");

        running_task.cancel.cancel();

        Ok(())
    }

    #[instrument(skip(self))]
    async fn show_tasks(&self, library: TaskLibrary) -> Result<Vec<Task>> {
        debug!("finding tasks");

        let mut out = Vec::new();

        match self.running_tasks.get(&library) {
            None => {}
            Some(entry) => match entry.read().await.as_ref() {
                None => {}
                Some(rt) => out.push(rt.task.clone()),
            },
        };

        match self.task_history.get(&library) {
            None => {}
            Some(entry) => {
                let ring = entry.read().await;

                let mut vec = ring.to_vec();
                vec.reverse();
                out.append(&mut vec);
            }
        };

        Ok(out)
    }

    #[instrument(skip(self))]
    async fn complete_task(
        &self,
        library: TaskLibrary,
        status: TaskStatus,
        warnings: Option<i64>,
        end: u64,
    ) -> Result<()> {
        // check if the task exists
        let rt_entry = self
            .running_tasks
            .get(&library)
            .ok_or_else(|| anyhow::Error::msg(format!("library {library} has no running task")))?;

        // hold the option lock just long enough to take() the running task
        let completed_task = {
            let mut running_task = rt_entry.write().await;

            // this should be the only place that tasks leave the running DashMap
            let completed_task = running_task.take().ok_or_else(|| {
                anyhow::Error::msg(format!("library {library} has no running task"))
            })?;

            debug!(
                { start = completed_task.task.start },
                "library task slot freed"
            );

            Result::<RunningTask>::Ok(completed_task)
        }?;

        // create the library's entry in the history task map if it doesn't exist
        //
        // this should be the only place that entries are put into the history DashMap,
        // all other calls should error somehow
        let ring_entry = match self.task_history.entry(library) {
            Entry::Occupied(entry) => entry.get().clone(),
            Entry::Vacant(entry) => {
                let v = Arc::new(RwLock::new(AllocRingBuffer::new(64)));
                entry.insert(v.clone());
                v
            }
        };

        // grab the ring buffer lock for the entirety of the archiving action
        let mut ring = ring_entry.write().await;

        ring.push(Task {
            task_type: completed_task.task.task_type,
            uid: completed_task.task.uid,
            status,
            warnings,
            start: completed_task.task.start,
            end: Some(end),
        });

        info!(
            { start = completed_task.task.start },
            "task saved to history"
        );

        Ok(())
    }
}

// task watcher
//
// this function ensures that our falliable tasks can be cancelled, and that
// the results are all correctly accounted for when sending the completion
// message back to the task service.
#[instrument(skip(task_future, sender))]
fn watch_task(
    start: u64,
    library: TaskLibrary,
    sender: EsmSender,
    task_future: Pin<Box<dyn Future<Output = Result<i64>> + Send>>,
) -> (JoinHandle<()>, CancellationToken) {
    let cancel = CancellationToken::new();

    let handle = {
        let cancel = cancel.clone();

        let task = async move {
            debug!("starting");

            let task_handle = spawn(task_future);

            let abort_handle = task_handle.abort_handle();

            debug!("waiting");

            let (status, warnings) = select! {
                _ = cancel.cancelled() => {
                    info!("aborting task");
                    abort_handle.abort();
                    (TaskStatus::Aborted, None)
                }

                res = task_handle => {
                    match res {
                        Ok(Ok(warnings)) => {
                            info!("task succeeded");
                            (TaskStatus::Success, Some(warnings))},
                        Ok(Err(err)) => {
                            error!("task failed: {err}");
                            (TaskStatus::Failure, None)
                        },
                        Err(_) => (TaskStatus::Unknown, None),
                    }

                }

            };

            // since the watcher future should not be able to fail, we collect all of the various
            // failure modes associated with sending the completion message and print their errors
            let (tx, rx) = tokio::sync::oneshot::channel();

            // this blocks on the write lock for the running task, since we need it to take() the
            // running task from the Option and put it into the ringbuffer
            match async {
                sender
                    .send(
                        TaskMsg::CompleteTask {
                            resp: tx,
                            library,
                            status,
                            warnings,
                            end: unix_time(),
                        }
                        .into(),
                    )
                    .await?;

                rx.await??;

                Result::<()>::Ok(())
            }
            .await
            {
                Ok(_) => {}
                Err(err) => error!("failed to send/receive completion message: {err}"),
            }
        };

        spawn(task)
    };

    (handle, cancel)
}
