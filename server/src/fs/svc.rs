use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_cell::sync::AsyncCell;
use async_trait::async_trait;
use chrono::Local;
use tokio::sync::{Mutex, RwLock};

use crate::db::msg::DbMsg;
use crate::fs::{msg::*, scan::*, ESFileService};
use crate::service::{ESInner, ESMReceiver, ESMSender, EntanglementService, ServiceType, ESM};
use api::library::{LibraryScanJob, LibraryUuid, LibraryUpdate};
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

    fn create(config: Arc<ESConfig>, sender_map: &mut HashMap<ServiceType, ESMSender>) -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel::<ESM>(1024);

        sender_map.insert(ServiceType::Fs, tx);

        FileService {
            config: config.clone(),
            receiver: Arc::new(Mutex::new(rx)),
            handle: AsyncCell::new(),
        }
    }

    async fn start(&self, senders: &HashMap<ServiceType, ESMSender>) -> anyhow::Result<()> {
        // falliable stuff can happen here

        let receiver = Arc::clone(&self.receiver);
        let state = Arc::new(FileScanner::new(self.config.clone(), senders.clone())?);

        let serve = {
            async move {
                let mut receiver = receiver.lock().await;

                while let Some(msg) = receiver.recv().await {
                    let state = Arc::clone(&state);
                    tokio::task::spawn(async move {
                        match state.message_handler(msg).await {
                            Ok(()) => (),
                            Err(_) => println!("file_service failed to reply to message"),
                        }
                    });
                }

                Err(anyhow::Error::msg(format!("channel disconnected")))
            }
        };

        let handle = tokio::task::spawn(serve);

        self.handle.set(handle);

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
    db_svc_sender: ESMSender,
    running_scans: Arc<RwLock<HashMap<LibraryUuid, Arc<RwLock<LibraryScanJob>>>>>,
}

#[async_trait]
impl ESFileService for FileScanner {
    async fn scan_library(&self, library_uuid: LibraryUuid) -> anyhow::Result<()> {
        let (library_tx, library_rx) = tokio::sync::oneshot::channel();

        self.db_svc_sender
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

        let context = Arc::new(ScanContext {
            config: self.config.clone(),
            library_uuid: library_uuid,
            library_path: PathBuf::from(library.path),
            db_svc_sender: self.db_svc_sender.clone(),
            media_linkdir: self.config.media_srvdir.clone(),
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

        self.db_svc_sender
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

#[async_trait]
impl ESInner for FileScanner {
    fn new(
        config: Arc<ESConfig>,
        senders: HashMap<ServiceType, ESMSender>,
    ) -> anyhow::Result<Self> {
        Ok(FileScanner {
            config: config.clone(),
            db_svc_sender: senders.get(&ServiceType::Db).unwrap().clone(),
            running_scans: Arc::new(RwLock::new(HashMap::new())),
        })
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
