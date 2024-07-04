use std::path::PathBuf;

use anyhow;

use async_trait::async_trait;

use crate::service::{ESInner, ESMResp, ESMSender};

pub mod msg;
pub mod svc;

#[async_trait]
pub trait ESFileService: ESInner {
    async fn scan_library(&self, resp: ESMResp<()>, library: String) -> anyhow::Result<()>;

    async fn rescan_file(&self, resp: ESMResp<()>, file: PathBuf) -> anyhow::Result<()>;
}

// should be called when
fn get_user_directory(user: String, root: PathBuf) -> anyhow::Result<PathBuf> {
    let mut root = root.clone();

    if !root.is_absolute() {
        return Err(anyhow::Error::msg("root path must be absolute"));
    }

    root.push("user_libraries");
    root.push(user);

    Ok(root)
}

fn scan_directory(dir: PathBuf) -> anyhow::Result<()> {
    Ok(())
}

fn record_file(db_sender: ESMSender, file: PathBuf) -> anyhow::Result<()> {
    Ok(())
}
