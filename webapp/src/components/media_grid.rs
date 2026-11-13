use std::collections::{HashMap, HashSet};

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    common::{
        colors::CollectionColor,
        paging::Pager,
        storage::{set_local_storage, try_local_storage},
    },
    components::{infinite::InfiniteScrollSentinel, media_card::MediaCard},
};
use api::{collection::CollectionUuid, media::MediaUuid, search::SearchResponse};

// thumbnail grid density, shared by every MediaGrid (gallery, collection and library
// detail) and remembered per browser.  comfortable is the original 280px-minimum
// layout; compact drops the minimum to 240px, uses a flatter 16:10 crop and
// tightens the card, which turns the usual two-column view into three columns and
// fits two rows on screen.
const GRID_DENSITY_KEY: &str = "media_grid_density";

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub enum GridDensity {
    #[default]
    Comfortable,
    Compact,
}

impl GridDensity {
    fn grid_class(self) -> &'static str {
        match self {
            Self::Comfortable => "media-grid",
            Self::Compact => "media-grid compact",
        }
    }
}

pub static GRID_DENSITY: GlobalSignal<GridDensity> =
    Signal::global(|| try_local_storage(GRID_DENSITY_KEY));

// compact icon menu for the page headers: a grid icon that opens a small popover
// with the two densities.  closes on selection, Escape, or a click anywhere else
// (via a transparent full-viewport backdrop behind the popover).
#[component]
pub fn GridDensityMenu() -> Element {
    let mut open = use_signal(|| false);
    let current = GRID_DENSITY();

    let item = move |density: GridDensity, label: &'static str| {
        let active = current == density;
        rsx! {
            button {
                r#type: "button",
                class: "menu-item",
                role: "menuitemradio",
                aria_checked: if active { "true" } else { "false" },
                onclick: move |_| {
                    *GRID_DENSITY.write() = density;
                    set_local_storage(GRID_DENSITY_KEY, density);
                    open.set(false);
                },
                span { class: "menu-item-check", aria_hidden: "true",
                    if active {
                        "✓"
                    }
                }
                "{label}"
            }
        }
    };

    rsx! {
        div {
            class: "grid-density-menu",
            onkeydown: move |evt| {
                if evt.key() == Key::Escape {
                    open.set(false);
                }
            },
            button {
                r#type: "button",
                class: "icon-btn",
                title: "Thumbnail size",
                aria_label: "Thumbnail size",
                aria_haspopup: "menu",
                aria_expanded: if open() { "true" } else { "false" },
                onclick: move |_| open.toggle(),
                // 2x2 grid glyph, drawn in currentColor so it follows the theme
                svg {
                    width: "18",
                    height: "18",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    "aria-hidden": "true",
                    rect { x: "3", y: "3", width: "7", height: "7", rx: "1" }
                    rect { x: "14", y: "3", width: "7", height: "7", rx: "1" }
                    rect { x: "3", y: "14", width: "7", height: "7", rx: "1" }
                    rect { x: "14", y: "14", width: "7", height: "7", rx: "1" }
                }
            }
            if open() {
                div { class: "menu-backdrop", onclick: move |_| open.set(false) }
                div {
                    class: "menu-popover",
                    role: "menu",
                    aria_label: "Thumbnail size",
                    div { class: "menu-label", "Thumbnail size" }
                    {item(GridDensity::Comfortable, "Large")}
                    {item(GridDensity::Compact, "Compact")}
                }
            }
        }
    }
}

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
    let grid_class = GRID_DENSITY().grid_class();

    if !initialized {
        return rsx! {
            div { class: "loading-state {grid_class}",
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
        div { class: grid_class,
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
