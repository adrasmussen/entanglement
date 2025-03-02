use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::{common::stream::thumbnail_link, components::modal::{Modal, MODAL_STACK}, Route};
use api::media::*;

#[derive(Clone, PartialEq, Props)]
pub struct MediaCardProps {
    media_uuid: MediaUuid,
    // Optional props for additional features
    #[props(default)]
    album_uuid: Option<i64>,
    #[props(default)]
    show_actions: bool,
}

#[component]
pub fn MediaCard(props: MediaCardProps) -> Element {
    let media_uuid = props.media_uuid;

    // If album_uuid is provided, we'll store it in local storage when clicked
    let album_context = props.album_uuid.map(|uuid| uuid.to_string());

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
                                if let Some(album_id) = &album_context {
                                    crate::common::storage::set_local_storage(
                                        crate::gallery::GALLERY_ALBUM_KEY,
                                        album_id.clone(),
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
                        // Optional action buttons
                        if props.show_actions {
                            div { class: "media-card-actions",
                                button {
                                    class: "btn btn-sm btn-secondary",
                                    onclick: move |_| {
                                        MODAL_STACK
                                            .with_mut(|v| v.push(Modal::ShowMedia(media_uuid)));
                                    },
                                    "View"
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
