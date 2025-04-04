use std::{pin::Pin, sync::Arc};

use anyhow::Result;
use async_cell::sync::AsyncCell;
use async_trait::async_trait;
use chrono::Local;
use dashmap::{DashMap, Entry};
use ringbuffer::{AllocRingBuffer, RingBuffer};
use tokio::{
    sync::{Mutex, RwLock},
    task::{spawn, JoinHandle},
};
use tracing::{debug, error, info, instrument, span, Instrument, Level};

use crate::{
    db::msg::DbMsg,
    service::{ESInner, ESMReceiver, ESMRegistry, EntanglementService, ServiceType, ESM},
    task::{msg::TaskMsg, scan::scan_library, sleep_task, ESTaskService},
};
use api::{
    library::LibraryUuid,
    task::{Task, TaskStatus, TaskType, TaskUid},
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
    // has an extra layer of abstraction (Option<_>) so that we can hold the lock
    // until the task successfully starts without blocking the DashMap
    running_tasks: DashMap<LibraryUuid, Arc<RwLock<Option<RunningTask>>>>,
    task_history: DashMap<LibraryUuid, Arc<RwLock<AllocRingBuffer<Task>>>>,
}

#[derive(Debug)]
struct RunningTask {
    task: Task,
    cancel: tokio::sync::mpsc::Sender<()>,
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
                TaskMsg::ShowTasks { resp, library_uuid } => {
                    self.respond(resp, self.show_tasks(library_uuid)).await
                }
                TaskMsg::CompleteTask {
                    resp,
                    library_uuid,
                    status,
                    warnings,
                    end,
                } => {
                    self.respond(
                        resp,
                        self.complete_task(library_uuid, status, warnings, end),
                    )
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
        library_uuid: LibraryUuid,
        task_type: TaskType,
        uid: TaskUid,
    ) -> Result<()> {
        debug!("task pre-startup verification");

        // library verification
        let db_svc_sender = self.registry().get(&ServiceType::Db)?;

        let (db_tx, db_rx) = tokio::sync::oneshot::channel();

        db_svc_sender
            .send(
                DbMsg::GetLibrary {
                    resp: db_tx,
                    library_uuid: library_uuid,
                }
                .into(),
            )
            .await?;

        db_rx.await??;

        // create the library's entry in the running task map if it doesn't exist
        //
        // this should be the only place that entries are put into the running DashMap,
        // all other calls should error somehow
        let rt_entry = match self.running_tasks.entry(library_uuid) {
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

        match *running_task {
            Some(_) => return Err(anyhow::Error::msg("task already running")),
            None => {}
        }

        let start = Local::now().timestamp();

        let task = Task {
            task_type: task_type.clone(),
            uid: uid,
            status: TaskStatus::Running,
            warnings: None,
            start: start,
            end: None,
        };

        debug!({task = ?task}, "created new task struct");

        // to abort the task, we can't simply drop the handle -- we need to explicitly call abort()
        // on either it or the associated abort handle.  thus, we create this channel and package it
        // as part of the tracked state to connect with the twice-separated running task future
        //
        // this an mpsc channel instead of oneshot so that we can clone the sender out of the struct
        // easily (no extra Option<> layer or similar)
        let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(64);

        let sender = self.registry().get(&ServiceType::Task)?;

        // task futures
        //
        // each task is a Future<Output=Result<T>> that is spawned into the executor by a separate
        // "watcher" future which uses the select! macro to await either the future or a cancellation
        // signal carried by the above channel
        //
        // currently, T = i64, the number of non-fatal errors encountered by the task.  in the future,
        // it could be a Box<dyn TaskReport> or similar
        //
        // what "failed" means depends on the task -- since tasks should typically produce reasonable
        // tracing logs, failure could either be catastrophic failure or a single error
        let task_future: Pin<Box<dyn futures::Future<Output = Result<i64>> + Send>> =
            match task_type {
                TaskType::ScanLibrary => Box::pin(scan_library(
                    self.config.clone(),
                    self.registry.clone(),
                    library_uuid,
                )),
                TaskType::RunScripts => Box::pin(sleep_task(library_uuid)),
                _ => return Err(anyhow::Error::msg("unsupported task")),
            };

        info!({ start = start }, "starting task");

        // watcher thread
        //
        // this is a wrapper future around the actual task, which lets us use tokio::select!
        // to either await its completion or cancel, and send a message either way
        let watcher = async move {
            debug!("task watcher starting");

            let task_handle = spawn(task_future);

            // we have to create an abort handle because select! takes ownership of the future
            // associated with the join handle
            let abort_handle = task_handle.abort_handle();

            debug!("task watcher waiting");
            let (status, warnings) = tokio::select! {
                _ = rx.recv() => {
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
            // failure modes and print their errors
            let (tx, rx) = tokio::sync::oneshot::channel();

            match async {
                sender
                    .send(
                        TaskMsg::CompleteTask {
                            resp: tx,
                            library_uuid: library_uuid,
                            status: status,
                            warnings,
                            end: Local::now().timestamp(),
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
        }
        .instrument(span!(Level::INFO, "task_watcher", start = start));

        // strictly speaking, we're not using this handle for anything and dropping it does not
        // abort the future.  however, it's possible we will need it later for shutdown logic
        let handle = spawn(watcher);

        *running_task = Some(RunningTask {
            task: task,
            cancel: tx,
            _handle: handle,
        });

        Ok(())
    }

    #[instrument(skip(self))]
    async fn stop_task(&self, library_uuid: LibraryUuid) -> Result<()> {
        let rt_entry = self.running_tasks.get(&library_uuid).ok_or_else(|| {
            anyhow::Error::msg(format!("library {library_uuid} has no running task"))
        })?;

        // the cancel channel requires a mutable borrow, so we need the write() lock
        let mut running_task = rt_entry.write().await;

        let running_task = running_task.as_mut().ok_or_else(|| {
            anyhow::Error::msg(format!("library {library_uuid} has no running task"))
        })?;

        info!({ start = running_task.task.start }, "stopping task");

        running_task.cancel.send(()).await.map_err(|err| {
            error!({library_uuid = library_uuid, task_type = %running_task.task.task_type, start = running_task.task.start}, "failed to send cancellation message to task");
            anyhow::Error::msg(format!("failed to send cancellation message to task: {err}"))})
    }

    #[instrument(skip(self))]
    async fn show_tasks(&self, library_uuid: LibraryUuid) -> Result<Vec<Task>> {
        debug!("finding tasks");

        let mut out = Vec::new();

        match self.running_tasks.get(&library_uuid) {
            None => {}
            Some(entry) => match entry.read().await.as_ref() {
                None => {}
                Some(rt) => out.push(rt.task.clone()),
            },
        };

        match self.task_history.get(&library_uuid) {
            None => {}
            Some(entry) => {
                let ring = entry.read().await;

                let mut vec = ring.to_vec();
                vec.reverse();
                out.append(&mut vec);
                //out.append(&mut ring.iter().map(|e| e.clone()).collect::<Vec<Task>>())
            }
        };

        Ok(out)
    }

    #[instrument(skip(self))]
    async fn complete_task(
        &self,
        library_uuid: LibraryUuid,
        status: TaskStatus,
        warnings: Option<i64>,
        end: i64,
    ) -> Result<()> {
        // check if the task exists
        let rt_entry = self.running_tasks.get(&library_uuid).ok_or_else(|| {
            anyhow::Error::msg(format!("library {library_uuid} has no running task"))
        })?;

        // hold the option lock just long enough to take() the running task from the mutex
        let completed_task = {
            let mut running_task = rt_entry.write().await;

            // this should be the only place that tasks leave the running DashMap
            let completed_task = running_task.take().ok_or_else(|| {
                anyhow::Error::msg(format!("library {library_uuid} has no running task"))
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
        let ring_entry = match self.task_history.entry(library_uuid) {
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
            status: status,
            warnings: warnings,
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
