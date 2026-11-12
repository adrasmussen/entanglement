use std::collections::{HashMap, HashSet};

use dioxus::prelude::*;

use crate::{
    common::{colors::CollectionColor, paging::Pager},
    components::{infinite::InfiniteScrollSentinel, media_card::MediaCard},
};
use api::{collection::CollectionUuid, media::MediaUuid, search::SearchResponse};

#[derive(Clone, PartialEq, Props)]
pub struct MediaGridProps {
    items: Signal<Vec<SearchResponse>>,
    pager: Pager,
    bulk_edit_signal: Signal<Option<HashSet<MediaUuid>>>,
    collection_color_signal: Signal<HashMap<CollectionUuid, CollectionColor>>,
    #[props(default)]
    collection_uuid: Option<CollectionUuid>,
    // page-specific empty state
    empty: Element,
}

#[component]
pub fn MediaGrid(props: MediaGridProps) -> Element {
    let items = props.items;
    let pager = props.pager;

    let initialized = (pager.initialized)();
    let error = (pager.error)();
    let is_empty = items.read().is_empty();

    if !initialized {
        return rsx! {
            div { class: "loading-state media-grid",
                for _ in 0..8 {
                    div { class: "skeleton-card",
                        div { class: "skeleton", style: "height: 200px;" }
                        div {
                            class: "skeleton",
                            style: "height: 24px; width: 40%; margin-top: 12px;",
                        }
                        div {
                            class: "skeleton",
                            style: "height: 18px; width: 80%; margin-top: 8px;",
                        }
                    }
                }
            }
        };
    }

    if is_empty {
        return match error {
            Some(err) => rsx! {
                div { class: "error-state",
                    p { "Error: {err}" }
                    button {
                        class: "btn btn-secondary",
                        onclick: move |_| pager.load_more.call(()),
                        "Retry"
                    }
                }
            },
            None => props.empty,
        };
    }

    rsx! {
        div { class: "media-grid",
            for search_resp in items.read().iter() {
                MediaCard {
                    key: "{search_resp.media_uuid}",
                    media_uuid: search_resp.media_uuid,
                    media: search_resp.media.clone(),
                    collections: search_resp.collections.clone(),
                    collection_uuid: props.collection_uuid,
                    bulk_edit_signal: props.bulk_edit_signal,
                    collection_color_signal: props.collection_color_signal,
                }
            }
        }
        InfiniteScrollSentinel { pager }
    }
}
