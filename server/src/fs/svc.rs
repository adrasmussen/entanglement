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
use common::{
    api::library::{LibraryScanJob, LibraryUuid},
    config::ESConfig,
};

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

    fn create(config: Arc<ESConfig>) -> (ESMSender, Self) {
        let (tx, rx) = tokio::sync::mpsc::channel::<ESM>(1024);

        (
            tx,
            FileService {
                config: config.clone(),
                receiver: Arc::new(Mutex::new(rx)),
                handle: AsyncCell::new(),
            },
        )
    }

    async fn start(&self, senders: HashMap<ServiceType, ESMSender>) -> anyhow::Result<()> {
        // falliable stuff can happen here

        let receiver = Arc::clone(&self.receiver);
        let state = Arc::new(FileScanner::new(self.config.clone(), senders)?);

        let serve = {
            async move {
                while let Some(msg) = receiver.lock().await.recv().await {
                    let state = Arc::clone(&state);
                    tokio::task::spawn(async move {
                        match state.message_handler(msg).await {
                            Ok(()) => (),
                            Err(_) => println!("file_service failed to reply to message"),
                        }
                    });
                }

                Err::<(), anyhow::Error>(anyhow::Error::msg(format!("channel disconnected")))
            }
        };

        let handle = tokio::task::spawn(serve);

        self.handle.set(handle);

        Ok(())
    }
}

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
            media_linkdir: self.config.media_linkdir.clone(),
            job_status: job,
        });

        run_scan(context).await;

        {
            let mut running_scans = self.running_scans.write().await;

            running_scans.remove(&library_uuid);
        }

        Ok(())
    }

    async fn scan_status(&self) -> anyhow::Result<HashMap<LibraryUuid, LibraryScanJob>> {
        let running_scans = self.running_scans.read().await.clone();

        let mut output = HashMap::new();

        for (k, v) in running_scans.iter() {
            output.insert(k.clone(), v.read().await.clone());
        }

        Ok(output)
    }

    async fn fix_symlinks(&self) -> anyhow::Result<()> {
        todo!()
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
                FsMsg::Status { resp } => self.respond(resp, async { Ok(()) }).await,
                FsMsg::ScanLibrary { resp, library_uuid } => {
                    self.respond(resp, self.scan_library(library_uuid)).await
                }
                FsMsg::ScanStatus { resp } => self.respond(resp, self.scan_status()).await,
                FsMsg::FixSymlinks { resp } => self.respond(resp, self.fix_symlinks()).await,
            },
            _ => Err(anyhow::Error::msg("not implemented")),
        }
    }
}
