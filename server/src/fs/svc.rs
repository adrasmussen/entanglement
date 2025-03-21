use std::collections::HashMap;

use std::sync::Arc;

use async_cell::sync::AsyncCell;
use async_trait::async_trait;
use chrono::Local;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, instrument, Level};

use crate::db::msg::DbMsg;
use crate::fs::{msg::*, scan::*, ESFileService};
use crate::service::{ESInner, ESMReceiver, ESMRegistry, EntanglementService, ServiceType, ESM};
use api::library::{LibraryScanJob, LibraryUpdate, LibraryUuid};
use common::config::ESConfig;

// file service
//
// the file service handles all of the operations invovling finding media, processing
// files, organizing the media directory, and so on
pub struct FileService {
    config: Arc<ESConfig>,
    receiver: Arc<Mutex<ESMReceiver>>,
    handle: AsyncCell<tokio::task::JoinHandle<anyhow::Result<()>>>,
}

#[async_trait]
impl EntanglementService for FileService {
    type Inner = FileScanner;

    fn create(config: Arc<ESConfig>, sender_map: &ESMRegistry) -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel::<ESM>(1024);

        sender_map
            .insert(ServiceType::Fs, tx)
            .expect("failed to insert sender for file service");

        FileService {
            config: config.clone(),
            receiver: Arc::new(Mutex::new(rx)),
            handle: AsyncCell::new(),
        }
    }

    #[instrument(level=Level::DEBUG, skip(self, registry))]
    async fn start(&self, registry: &ESMRegistry) -> anyhow::Result<()> {
        info!("starting legacy file service");

        let receiver = Arc::clone(&self.receiver);
        let state = Arc::new(FileScanner::new(self.config.clone(), registry.clone())?);

        let serve = {
            async move {
                let mut receiver = receiver.lock().await;

                while let Some(msg) = receiver.recv().await {
                    let state = Arc::clone(&state);
                    tokio::task::spawn(async move {
                        match state.message_handler(msg).await {
                            Ok(()) => (),
                            Err(err) => {
                                error!({service = "file_service", channel = "esm", error = %err})
                            }
                        }
                    });
                }

                Err(anyhow::Error::msg(format!(
                    "file_service esm channel disconnected"
                )))
            }
        };

        let handle = tokio::task::spawn(serve);

        self.handle.set(handle);

        debug!("started legacy file service");
        Ok(())
    }
}

// filescanner
//
// this is the inner service that knows how to dispatch and eventually monitor scans,
// although the first implementation is pretty basic
//
// it is missing, at minimum, a way to stop a scan (and all scans), ways to check that
// the handles haven't expired, and so on
pub struct FileScanner {
    config: Arc<ESConfig>,
    registry: ESMRegistry,
    running_scans: Arc<RwLock<HashMap<LibraryUuid, Arc<RwLock<LibraryScanJob>>>>>,
}

#[async_trait]
impl ESInner for FileScanner {
    fn new(config: Arc<ESConfig>, registry: ESMRegistry) -> anyhow::Result<Self> {
        Ok(FileScanner {
            config: config.clone(),
            registry: registry.clone(),
            running_scans: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    fn registry(&self) -> ESMRegistry {
        self.registry.clone()
    }

    async fn message_handler(&self, esm: ESM) -> anyhow::Result<()> {
        match esm {
            ESM::Fs(message) => match message {
                FsMsg::_Status { resp } => {
                    self.respond(resp, async { Err(anyhow::Error::msg("not implemented")) })
                        .await
                }
                FsMsg::ScanLibrary { resp, library_uuid } => {
                    self.respond(resp, self.scan_library(library_uuid)).await
                }
                FsMsg::ScanStatus { resp } => self.respond(resp, self.scan_status()).await,
                FsMsg::StopScan { resp, library_uuid } => {
                    self.respond(resp, self.stop_scan(library_uuid)).await
                }
                FsMsg::FixSymlinks { resp } => self.respond(resp, self.fix_symlinks()).await,
            },
            _ => Err(anyhow::Error::msg("not implemented")),
        }
    }
}

#[async_trait]
impl ESFileService for FileScanner {
    async fn scan_library(&self, library_uuid: LibraryUuid) -> anyhow::Result<()> {
        let db_svc_sender = self.registry.get(&ServiceType::Db)?;
        let (library_tx, library_rx) = tokio::sync::oneshot::channel();

        db_svc_sender
            .send(
                DbMsg::GetLibrary {
                    resp: library_tx,
                    library_uuid: library_uuid.clone(),
                }
                .into(),
            )
            .await?;

        let library = library_rx.await??.ok_or_else(|| {
            anyhow::Error::msg(format!("Failed to find library {}", library_uuid.clone()))
        })?;

        // rather than start another thread to listen for the jobs status, we create this shared
        // struct that will be arc'd into the scan context, modified over the couse of the scan,
        // and read back later to send the status/library update
        //
        // TODO -- convert this to a channel or clarify the ownership scheme
        let job = Arc::new(RwLock::new(LibraryScanJob {
            start_time: Local::now().timestamp(),
            file_count: 0,
            error_count: 0,
            status: "intializing scan".to_owned(),
        }));

        {
            let mut running_scans = self.running_scans.write().await;

            match running_scans.insert(library_uuid, job.clone()) {
                None => {}
                Some(v) => {
                    running_scans.insert(library_uuid, v);
                    return Err(anyhow::Error::msg(format!(
                        "library scan for {library_uuid} already in progress"
                    )));
                }
            }
        }

        // this struct contains everything needed to actually run a scan, including the job struct,
        // conveniently packaged so that run_scan() can easily pass relevant information down
        let context = Arc::new(ScanContext {
            config: self.config.clone(),
            library_uuid: library_uuid,
            library_path: self.config.media_srcdir.clone().join(library.path),
            db_svc_sender: db_svc_sender.clone(),
            job_status: job,
        });

        run_scan(context).await;

        // TODO -- figure out how to have the post-processing steps identify if the scan stopped due to error
        // or signal, and report appropriately
        let count = {
            let mut running_scans = self.running_scans.write().await;

            let count = running_scans
                .get(&library_uuid)
                .ok_or_else(|| anyhow::Error::msg("scan completed but job missing from manager"))?
                .read()
                .await
                .file_count
                .clone();

            running_scans.remove(&library_uuid);

            count
        };

        let (update_tx, update_rx) = tokio::sync::oneshot::channel();

        db_svc_sender
            .send(
                DbMsg::UpdateLibrary {
                    resp: update_tx,
                    library_uuid: library_uuid,
                    update: LibraryUpdate { count: Some(count) },
                }
                .into(),
            )
            .await?;

        update_rx.await?
    }

    async fn scan_status(&self) -> anyhow::Result<HashMap<LibraryUuid, LibraryScanJob>> {
        let running_scans = self.running_scans.read().await.clone();

        let mut output = HashMap::new();

        for (k, v) in running_scans.iter() {
            output.insert(k.clone(), v.read().await.clone());
        }

        Ok(output)
    }

    async fn stop_scan(&self, library_uuid: LibraryUuid) -> anyhow::Result<()> {
        Err(anyhow::Error::msg(format!(
            "not implemented for {library_uuid}"
        )))
    }

    async fn fix_symlinks(&self) -> anyhow::Result<()> {
        Err(anyhow::Error::msg("not implemented"))
    }
}
