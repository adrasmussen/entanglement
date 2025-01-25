use api::{THUMBNAIL_PATH, ORIGINAL_PATH, media::MediaUuid};

// note -- find a way to pull the build data in here,
// since these technically need to match the api server
pub fn full_link(media_uuid: MediaUuid) -> String {
    format!("/entanglement/media/{ORIGINAL_PATH}/{media_uuid}")
}

pub fn thumbnail_link(media_uuid: MediaUuid) -> String {
    format!("/entanglement/media/{THUMBNAIL_PATH}/{media_uuid}")
}
