use std::collections::HashSet;

use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::{common::storage::set_local_storage, gallery::GALLERY_COLLECTION_KEY, Route};
use api::{collection::CollectionUuid, media::*, thumbnail_link};

// TODO -- deduplicate the error handling in the the callsites by making a MediaGrid
// with an error boundary
#[derive(Clone, PartialEq, Props)]
pub struct MediaCardProps {
    media_uuid: MediaUuid,
    media: Media,
    collections: Vec<CollectionUuid>,
    bulk_edit_mode_signal: Signal<bool>,
    selected_media_signal: Signal<HashSet<MediaUuid>>,
    // Optional props for additional features
    #[props(default)]
    collection_uuid: Option<i64>,
}

#[component]
pub fn MediaCard(props: MediaCardProps) -> Element {
    let media_uuid = props.media_uuid;
    let media = props.media;
    //let collections = props.collections;

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

    rsx! {
        div {
            class: "media-card",
            style: if bulk_edit_mode_signal() && is_selected { "position: relative; border: 3px solid var(--primary); transform: scale(0.98);" } else { "position: relative;" },
            // Add selection overlay in bulk edit mode
            if bulk_edit_mode_signal() {
                div {
                    style: "position: absolute; top: 0; left: 0; width: 100%; height: 100%; z-index: 10; cursor: pointer;",
                    onclick: toggle_selection,
                    // somewhat inexplicably, this is the radio button in the corner of the tile
                    div { style: "position: absolute; top: 10px; right: 10px; width: 24px; height: 24px; border-radius: 50%; background-color: var(--surface); border: 2px solid var(--primary); display: flex; align-items: center; justify-content: center;",
                        if is_selected {
                            div { style: "width: 12px; height: 12px; background-color: var(--primary); border-radius: 50%;" }
                        }
                    }
                }
            }

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
                        alt: if media.note.is_empty() { format!("Media {}", media_uuid) } else { media.note.clone() },
                        loading: "lazy",
                    }
                }
                div { class: "media-card-info",
                    p { class: "date", "{media.date}" }
                    p { class: "note",
                        if media.note.is_empty() {
                            "No description"
                        } else {
                            {media.note.clone().lines().next().unwrap_or_else(|| "No description")}
                        }
                    }
                }
            }

        }
    }
}
