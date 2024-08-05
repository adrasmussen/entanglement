use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{self, Context};

use async_trait::async_trait;

use tokio::sync::Mutex;

use crate::db::msg::DbMsg;
use crate::fs::{msg::*, scan::*, *};

pub struct FileScanner {
    db_svc_sender: ESMSender,
    media_linkdir: PathBuf,
}

#[async_trait]
impl ESFileService for FileScanner {
    async fn scan_library(&self, library_uuid: LibraryUuid) -> anyhow::Result<ScanReport> {
        // first, get the library details
        let (library_tx, library_rx) = tokio::sync::oneshot::channel();

        self.db_svc_sender
            .send(
                DbMsg::GetLibary {
                    resp: library_tx,
                    uuid: library_uuid.clone(),
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

        Ok(ScanReport {
            count: scan_count.clone(),
            errors: scan_errors.clone(),
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
