use std::path::PathBuf;

use anyhow::Result;
use hex::encode;
use sha2::{Digest, Sha512};
use tokio::{
    fs::File,
    io::{AsyncReadExt, BufReader},
};

use api::media::MediaMetadata;

pub mod image;
pub mod video;

// intermediate struct used by media processing functions
#[derive(Clone, Debug)]
pub struct MediaData {
    pub hash: String,
    pub date: String,
    pub metadata: MediaMetadata,
}

pub async fn content_hash(path: &PathBuf) -> Result<String> {
    let file = File::open(&path).await?;

    let mut hasher = Sha512::new();
    let mut buffer = [0; 8192];

    // TODO -- perf tuning
    let mut reader = BufReader::with_capacity(8182, file);

    while reader.read(&mut buffer).await? > 0 {
        hasher.update(buffer);
    }

    Ok(encode(hasher.finalize()))
}
