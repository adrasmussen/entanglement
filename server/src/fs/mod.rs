use std::path::PathBuf;

use anyhow;

use async_trait::async_trait;

use crate::service::{ESInner, ESMResp, ESMSender};

pub mod msg;
pub mod svc;

#[async_trait]
pub trait ESFileService: ESInner {
    async fn scan_library(&self, user: String) -> anyhow::Result<()>;

    async fn rescan_file(&self, file: PathBuf) -> anyhow::Result<()>;

    async fn fix_symlinks(&self) -> anyhow::Result<()>;
}

fn user_to_library(user: String, media_srcdir: PathBuf) -> anyhow::Result<PathBuf> {
    let mut root = media_srcdir.clone();

    if !root.is_absolute() {
        return Err(anyhow::Error::msg("media_srcdir path must be absolute"));
    }

    root.push("libraries");
    root.push(user);

    Ok(root)
}
