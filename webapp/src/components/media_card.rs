use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::Route;
use api::{thumbnail_link, media::*};

#[derive(Clone, PartialEq, Props)]
pub struct MediaCardProps {
    media_uuid: MediaUuid,
    // Optional props for additional features
    #[props(default)]
    collection_uuid: Option<i64>,
}

#[component]
pub fn MediaCard(props: MediaCardProps) -> Element {
    let media_uuid = props.media_uuid;

    // If collection_uuid is provided, we'll store it in local storage when clicked
    let collection_context = props.collection_uuid.map(|uuid| uuid.to_string());

    // Fetch media info to show preview metadata
    let media_info =
        use_resource(move || async move { get_media(&GetMediaReq { media_uuid }).await });

    rsx! {
        div { class: "media-card",
            match &*media_info.read() {
                Some(Ok(info)) => {
                    rsx! {
                        Link {
                            to: Route::GalleryDetail {
                                media_uuid: media_uuid.to_string(),
                            },
                            onclick: move |_| {
                                if let Some(collection_id) = &collection_context {
                                    crate::common::storage::set_local_storage(
                                        crate::gallery::GALLERY_COLLECTION_KEY,
                                        collection_id.clone(),
                                    );
                                }
                            },
                            div { class: "media-card-image",
                                img {
                                    src: thumbnail_link(media_uuid),
                                    alt: if info.media.note.is_empty() { format!("Media {}", media_uuid) } else { info.media.note.clone() },
                                    loading: "lazy",
                                }
                            }
                            div { class: "media-card-info",
                                p { class: "date", "{info.media.date}" }
                                p { class: "note",
                                    if info.media.note.is_empty() {
                                        "No description"
                                    } else {
                                        {info.media.note.clone()}
                                    }
                                }
                            }
                        }
                    }
                }
                Some(Err(_)) => {
                    rsx! {
                        div { class: "media-card-error", "Failed to load media information" }
                    }
                }
                None => {
                    rsx! {
                        div { class: "media-card-loading",
                            div { class: "skeleton", style: "height: 200px;" }
                            div { class: "media-card-info",
                                div { class: "skeleton", style: "width: 40%; margin-bottom: 8px;" }
                                div { class: "skeleton", style: "width: 80%;" }
                            }
                        }
                    }
                }
            }
        }
    }
}
