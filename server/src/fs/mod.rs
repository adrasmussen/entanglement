use std::{path::PathBuf, sync::Arc};

use api::{media::MediaUuid, LINK_PATH, THUMBNAIL_PATH};
use common::config::ESConfig;

// legacy file service
//
// originally, this module held the library scanner logic,
// but it was moved to the task module instead.
//
// since it is likely that there will be more server filesystem
// specific tasks, we leave the module in its own folder
pub fn media_link_path(config: Arc<ESConfig>, media_uuid: MediaUuid) -> PathBuf {
    config
        .fs
        .media_srvdir
        .join(LINK_PATH)
        .join(media_uuid.to_string())
}

pub fn media_thumbnail_path(config: Arc<ESConfig>, media_uuid: MediaUuid) -> PathBuf {
    config
        .fs
        .media_srvdir
        .join(THUMBNAIL_PATH)
        .join(media_uuid.to_string())
}
