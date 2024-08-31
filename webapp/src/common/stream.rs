use api::media::MediaUuid;

pub fn full_link(media_uuid: MediaUuid) -> String {
    format!("/entanglement/media/full/{media_uuid}")
}

pub fn thumbnail_link(media_uuid: MediaUuid) -> String {
    format!("/entanglement/media/thumbnails/{media_uuid}")
}
