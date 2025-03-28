use std::path::PathBuf;
use std::sync::Arc;

use api::THUMBNAIL_PATH;

use api::{media::MediaUuid, ORIGINAL_PATH};
use common::config::ESConfig;

// legacy file service
//
// originally, this module held the library scanner logic,
// but it was moved to the task module instead.
//
// since it is likely that there will be more server filesystem
// specific tasks, we leave the module in its own folder
pub fn media_original_path(config: Arc<ESConfig>, media_uuid: MediaUuid) -> PathBuf {
    config
        .media_srvdir
        .join(ORIGINAL_PATH)
        .join(media_uuid.to_string())
}

pub fn media_thumbnail_path(config: Arc<ESConfig>, media_uuid: MediaUuid) -> PathBuf {
    config
        .media_srvdir
        .join(THUMBNAIL_PATH)
        .join(media_uuid.to_string())
}
