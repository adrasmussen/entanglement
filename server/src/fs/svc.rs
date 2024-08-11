use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{self, Context};

use async_cell::sync::AsyncCell;

use async_trait::async_trait;

use chrono::offset::Local;

use tokio::sync::Mutex;

use crate::db::msg::DbMsg;
use crate::fs::{msg::*, scan::*, *};
use api::library::LibraryMetadata;

pub struct FileScanner {
    db_svc_sender: ESMSender,
    media_linkdir: PathBuf,
}

#[async_trait]
impl ESFileService for FileScanner {
    async fn scan_library(&self, library_uuid: LibraryUuid) -> anyhow::Result<LibraryScanResult> {
        // first, get the library details
        let (library_tx, library_rx) = tokio::sync::oneshot::channel();

        self.db_svc_sender
            .send(
                DbMsg::GetLibrary {
                    resp: library_tx,
                    library_uuid: library_uuid.clone(),
                }
                .into(),
            )
            .await
            .context("Failed to send GetLibrary message from scan_library")?;

        let library = library_rx
            .await
            .context("Failed to receive GetLibrary response at scan_library")??
            .ok_or_else(|| {
                anyhow::Error::msg(format!("Failed to find library {}", library_uuid.clone()))
            })?;

        // then set up the scanner error collection loop
        let scan_count = Arc::new(Mutex::new(i64::from(0)));
        let scan_errors = Arc::new(Mutex::new(Vec::new()));
        let (scan_tx, mut scan_rx) = tokio::sync::mpsc::channel(1024);

        let scan_info = Arc::new(ScanContext {
            scan_sender: scan_tx,
            library_uuid: library_uuid,
            db_svc_sender: self.db_svc_sender.clone(),
            media_linkdir: self.media_linkdir.clone(),
        });

        let _ = tokio::spawn({
            let scan_count = scan_count.clone();
            let scan_errors = scan_errors.clone();

            async move {
                while let Some(msg) = scan_rx.recv().await {
                    match msg {
                        Ok(()) => {
                            let mut scan_count = scan_count.lock().await;

                            *scan_count += 1;
                        }
                        Err(err) => {
                            let mut scan_errors = scan_errors.lock().await;

                            scan_errors.push(err);
                        }
                    }
                }
            }
        });

        scan_directory(scan_info, library.path.into())
            .await
            .map_err(|err| anyhow::Error::msg(err.info))?;

        let scan_count = scan_count.lock().await;
        let scan_errors = scan_errors.lock().await;

        let (update_tx, update_rx) = tokio::sync::oneshot::channel();

        self.db_svc_sender
            .send(
                DbMsg::UpdateLibrary {
                    resp: update_tx,
                    library_uuid: library_uuid.clone(),
                    change: LibraryMetadata {
                        file_count: scan_count.clone(),
                        last_scan: Local::now().timestamp(),
                    },
                }
                .into(),
            )
            .await
            .context("Failed to send UpdateLibrary message from scan_library")?;

        update_rx.await.context("Failed to receive UpdateLibrary response at scan_library")??;

        Ok(LibraryScanResult {
            count: scan_count.clone(),
            errors: scan_errors.clone().into_iter().map(|err| format!("{:?}: {}", err.path, err.info)).collect(),
        })
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
            db_svc_sender: senders.get(&ServiceType::Db).unwrap().clone(),
            media_linkdir: config.media_linkdir.clone(),
        })
    }

    async fn message_handler(&self, esm: ESM) -> anyhow::Result<()> {
        match esm {
            ESM::Fs(message) => match message {
                FsMsg::Status { resp } => self.respond(resp, async { todo!() }).await,
                FsMsg::ScanLibrary { resp, library_uuid } => {
                    self.respond(resp, self.scan_library(library_uuid)).await
                }
                FsMsg::FixSymlinks { resp } => self.respond(resp, self.fix_symlinks()).await,
            },
            _ => Err(anyhow::Error::msg("not implemented")),
        }
    }
}

pub struct FileService {
    config: Arc<ESConfig>,
    receiver: Arc<Mutex<ESMReceiver>>,
    handle: AsyncCell<tokio::task::JoinHandle<anyhow::Result<()>>>,
}

#[async_trait]
impl EntanglementService for FileService {
    type Inner = FileScanner;

    fn create(config: Arc<ESConfig>) -> (ESMSender, Self) {
        let (tx, rx) = tokio::sync::mpsc::channel::<ESM>(32);

        (
            tx,
            FileService {
                config: config.clone(),
                receiver: Arc::new(Mutex::new(rx)),
                handle: AsyncCell::new(),
            }
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
