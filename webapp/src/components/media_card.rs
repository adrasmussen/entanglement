use std::collections::{HashMap, HashSet};

use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::{
    Route,
    common::{colors::CollectionColor, storage::set_local_storage},
    gallery::GALLERY_COLLECTION_KEY,
};
use api::{collection::CollectionUuid, media::*, thumbnail_link};

// TODO -- deduplicate the error handling in the the callsites by making a MediaGrid
// with an error boundary
#[derive(Clone, PartialEq, Props)]
pub struct MediaCardProps {
    // media data fetched from the database
    media_uuid: MediaUuid,
    media: Media,
    collections: Vec<CollectionUuid>,
    // currently used for ham-fisted indication of which collection sent
    // the user to the media detail page
    #[props(default)]
    collection_uuid: Option<CollectionUuid>,
    // signals used by various other components
    bulk_edit_signal: Signal<Option<HashSet<MediaUuid>>>,
    collection_color_signal: Signal<HashMap<CollectionUuid, CollectionColor>>,
}

#[component]
pub fn MediaCard(props: MediaCardProps) -> Element {
    let media_uuid = props.media_uuid;
    let media = props.media;
    let collections = props.collections;

    let mut bulk_edit_signal = props.bulk_edit_signal;
    let collection_color_signal = props.collection_color_signal;

    let is_selected = bulk_edit_signal()
        .map(|s| s.contains(&media_uuid))
        .unwrap_or(false);

    let mut toggle_selection = move |evt: MouseEvent| {
        evt.prevent_default();
        evt.stop_propagation();

        bulk_edit_signal.with_mut(|set| {
            if let Some(set) = set {
                if set.contains(&media_uuid) {
                    set.remove(&media_uuid);
                } else {
                    set.insert(media_uuid);
                }
            }
        });
    };

    let collection_context = props.collection_uuid.map(|uuid| uuid.to_string());

    // Calculate collection colors for this media
    let collection_colors: Vec<CollectionColor> = collections
        .iter()
        .filter_map(|uuid| collection_color_signal().get(uuid).cloned())
        .collect();

    // Generate background style based on collection colors
    let info_background_style = if collection_colors.is_empty() {
        String::new()
    } else if collection_colors.len() == 1 {
        // Single color - use the light pastel version
        format!("background-color: {}; border-radius: 0 0 var(--radius-lg) var(--radius-lg);", collection_colors[0].to_medium_css_color())
    } else {
        // Multiple colors - create horizontal stripes using CSS gradient
        let stripe_size = 100.0 / collection_colors.len() as f32;
        let mut gradient_stops = Vec::new();

        for (i, color) in collection_colors.iter().enumerate() {
            let start_percent = i as f32 * stripe_size;
            let end_percent = (i + 1) as f32 * stripe_size;

            gradient_stops.push(format!(
                "{} {}%, {} {}%",
                color.to_medium_css_color(),
                start_percent,
                color.to_medium_css_color(),
                end_percent
            ));
        }

        format!(
            "background: linear-gradient(to right, {}); border-radius: 0 0 var(--radius-lg) var(--radius-lg);",
            gradient_stops.join(", ")
        )
    };

    rsx! {
        div {
            class: "media-card",
            style: if bulk_edit_signal().is_some() && is_selected { "position: relative; border: 3px solid var(--primary); transform: scale(0.98);" } else { "position: relative;" },
            // Add selection overlay in bulk edit mode
            if bulk_edit_signal().is_some() {
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
                    if bulk_edit_signal().is_some() {
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
                div { class: "media-card-info", style: info_background_style,

                    p { class: "date", "{media.date}" }
                    p { class: "note",
                        if media.note.is_empty() {
                            "No description"
                        } else {
                            {media.note.clone().lines().next().unwrap_or("No description").to_owned()}
                        }
                    }
                }
            }
        }
    }
}
