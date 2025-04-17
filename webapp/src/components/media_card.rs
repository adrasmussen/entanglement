use std::collections::HashSet;

use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::{common::storage::set_local_storage, gallery::GALLERY_COLLECTION_KEY, Route};
use api::{media::*, thumbnail_link};

#[derive(Clone, PartialEq, Props)]
pub struct MediaCardProps {
    media_uuid: MediaUuid,
    bulk_edit_mode_signal: Signal<bool>,
    selected_media_signal: Signal<HashSet<MediaUuid>>,
    // Optional props for additional features
    #[props(default)]
    collection_uuid: Option<i64>,
}

#[component]
pub fn MediaCard(props: MediaCardProps) -> Element {
    let media_uuid = props.media_uuid;
    let bulk_edit_mode_signal = props.bulk_edit_mode_signal;
    let mut selected_media_signal = props.selected_media_signal;

    let is_selected = selected_media_signal().contains(&media_uuid);

    let mut toggle_selection = move |evt: MouseEvent| {
        evt.prevent_default();
        evt.stop_propagation();

        selected_media_signal.with_mut(|set| {
            if set.contains(&media_uuid) {
                set.remove(&media_uuid);
            } else {
                set.insert(media_uuid);
            }
        });
    };

    let collection_context = props.collection_uuid.map(|uuid| uuid.to_string());

    // Fetch media info to show preview metadata
    let media_info =
        use_resource(move || async move { get_media(&GetMediaReq { media_uuid }).await });

    rsx! {
        div {
            class: "media-card",
            style: if bulk_edit_mode_signal() && is_selected { "position: relative; border: 3px solid var(--primary); transform: scale(0.98);" } else { "position: relative;" },
            // Add selection overlay in bulk edit mode
            if bulk_edit_mode_signal() {
                div {
                    onclick: toggle_selection,
                    style: "position: absolute; top: 0; left: 0; width: 100%; height: 100%; z-index: 10; cursor: pointer;",
                    div { style: "position: absolute; top: 10px; right: 10px; width: 24px; height: 24px; border-radius: 50%; background-color: var(--surface); border: 2px solid var(--primary); display: flex; align-items: center; justify-content: center;",
                        if is_selected {
                            div { style: "width: 12px; height: 12px; background-color: var(--primary); border-radius: 50%;" }
                        }
                    }
                }
            }

            match &*media_info.read() {
                Some(Ok(info)) => {
                    rsx! {
                        Link {
                            to: Route::GalleryDetail {
                                media_uuid: media_uuid.to_string(),
                            },
                            onclick: move |evt: MouseEvent| {
                                if bulk_edit_mode_signal() {
                                    evt.prevent_default();
                                    evt.stop_propagation();
                                    toggle_selection(evt);
                                } else if let Some(collection_id) = &collection_context {
                                    set_local_storage(GALLERY_COLLECTION_KEY, collection_id.clone());
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
