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
