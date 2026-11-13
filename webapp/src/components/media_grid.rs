use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    common::{
        colors::CollectionColor,
        grid_cache::{LayoutKey, WindowState, chunk_size},
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
    pub fn is_compact(self) -> bool {
        self == Self::Compact
    }

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

// windowing of the grid DOM.  the cards above the viewport are the memory cost of a long
// scroll (DOM plus decoded thumbnails; the item data itself is small), so once there are
// KEEP_ABOVE cards above the viewport, the topmost chunk of whole rows is unmounted and
// stood in for by a spacer of exactly the same height.  scrolling back up remounts them
// from the data we kept, with no request.  tune these to taste.
const KEEP_ABOVE: usize = 200;

const GRID_ID: &str = "media-grid";

// the number of columns the grid currently lays out
async fn measure_cols() -> Option<usize> {
    let mut eval = document::eval(&format!(
        r#"
        const g = document.getElementById("{GRID_ID}");
        dioxus.send(g
            ? getComputedStyle(g).gridTemplateColumns.split(" ").filter((s) => s).length
            : null);
        "#
    ));
    eval.recv::<Option<usize>>().await.ok().flatten()
}

// is the card `probe` places after the first mounted one entirely above the viewport?  if
// so, the distance in px between the first card and the `chunk`th one (rows and gaps
// included, which is exactly the height their spacer has to be)
async fn measure_trim(probe: usize, chunk: usize) -> Option<f64> {
    let mut eval = document::eval(&format!(
        r#"
        const g = document.getElementById("{GRID_ID}");
        dioxus.send(g && g.children.length > {probe} && g.children[{probe}].getBoundingClientRect().bottom < 0
            ? g.children[{chunk}].offsetTop - g.children[0].offsetTop
            : null);
        "#
    ));
    eval.recv::<Option<f64>>().await.ok().flatten()
}

// resolves once the browser has painted a frame, by which time any render we caused is done
async fn next_frame() {
    let mut eval = document::eval(
        "requestAnimationFrame(() => requestAnimationFrame(() => dioxus.send(true)));",
    );
    let _ = eval.recv::<bool>().await;
}

// unmount chunks of cards at the top for as long as more than KEEP_ABOVE (plus the chunk
// itself) of them are above the viewport
async fn trim_while_above(mut window: Signal<WindowState>) {
    loop {
        let before = window.peek().clone();

        // whole rows only, so the cards that remain keep their columns
        let Some(cols) = measure_cols().await.filter(|c| *c > 0) else {
            return;
        };
        let chunk = chunk_size(cols);
        let Some(px) = measure_trim(KEEP_ABOVE + chunk, chunk)
            .await
            .filter(|px| *px > 0.0)
        else {
            return;
        };

        // anything that happened while we were measuring invalidates the measurement
        if *window.peek() != before {
            return;
        }
        window.write().trim(chunk, px);

        // let the trim render before measuring again
        next_frame().await;
    }
}

// tells us (at most four times a second) when scrolling has put at least KEEP_ABOVE cards
// above the viewport.  an IntersectionObserver on a card can't do this, since it only
// reports changes and a fast scroll can carry the card from below the viewport to above it
// without it ever intersecting.  window.__gridWatchers[id] removes this grid's watcher, which
// is also how the grid cleans up after itself.  watchers are keyed by WATCH_ID, unique
// per mounted grid, so an outgoing grid's drop can never stop the incoming grid's watcher
// (the drop of one page and the mount of the next aren't ordered)
const WATCH_JS: &str = r#"
    window.__gridWatchers = window.__gridWatchers || {};
    let queued = false;
    let last = 0;
    const check = () => {
        queued = false;
        const g = document.getElementById("media-grid");
        if (g && g.children.length > KEEP_ABOVE
            && g.children[KEEP_ABOVE].getBoundingClientRect().bottom < 0) {
            last = performance.now();
            dioxus.send(true);
        }
    };
    const onScroll = () => {
        if (!queued && performance.now() - last > 250) {
            queued = true;
            requestAnimationFrame(check);
        }
    };
    window.addEventListener("scroll", onScroll, { passive: true });
    window.__gridWatchers["WATCH_ID"] = () => {
        window.removeEventListener("scroll", onScroll);
        delete window.__gridWatchers["WATCH_ID"];
    };
    onScroll();
"#;

#[derive(Clone, PartialEq, Props)]
pub struct MediaGridProps {
    items: Signal<Vec<SearchResponse>>,
    // which of the items are mounted, see KEEP_ABOVE
    window: Signal<WindowState>,
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
    let mut window = props.window;
    let pager = props.pager;

    let trimming = use_hook(|| Rc::new(Cell::new(false)));

    // heights measured in one density mean nothing in the other
    let compact = GRID_DENSITY().is_compact();
    use_effect(move || {
        let compact = GRID_DENSITY().is_compact();
        if window.peek().compact != compact {
            window.set(WindowState::fresh(LayoutKey::current()));
        }
    });

    // watch the scroll position for cards to unmount
    let watch_id = use_hook(|| {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    });
    use_hook({
        let trimming = trimming.clone();
        move || {
            let mut watcher = document::eval(
                &WATCH_JS
                    .replace("KEEP_ABOVE", &KEEP_ABOVE.to_string())
                    .replace("WATCH_ID", &watch_id.to_string()),
            );
            spawn(async move {
                while watcher.recv::<bool>().await.is_ok() {
                    if !trimming.replace(true) {
                        trim_while_above(window).await;
                        trimming.set(false);
                    }
                }
            });
        }
    });
    use_drop(move || {
        let _ = document::eval(&format!(
            "const s = window.__gridWatchers && window.__gridWatchers[\"{watch_id}\"]; if (s) s();"
        ));
    });

    // the spacer coming into view means the user is scrolling back up
    let on_spacer_visible = use_callback(move |evt: VisibleEvent| {
        if evt.data().is_intersecting().unwrap_or(false) {
            window.write().untrim();
        }
    });

    // the layout changed under the measured heights
    let on_resize = use_callback(move |evt: ResizeEvent| {
        let Ok(size) = evt.data().get_content_box_size() else {
            return;
        };
        let width = size.width;

        let known = window.peek().grid_width;
        if known == 0.0 {
            window.write().grid_width = width;
        } else if (known - width).abs() > 1.0 {
            let mut fresh = WindowState::fresh(LayoutKey::current());
            fresh.grid_width = width;
            window.set(fresh);
        }
    });

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

    // the effect above fixes the stored state after a density change; until it runs, nothing
    // is considered trimmed
    let (start, spacer_px) = {
        let w = window.read();
        if w.compact == compact {
            (w.start, w.spacer_px())
        } else {
            (0, 0.0)
        }
    };
    let list = items.read();
    let start = start.min(list.len());

    rsx! {
        // a new key per trim state gives the spacer a fresh observer, which reports its
        // visibility right away; one that is still on screen after being shrunk keeps
        // pulling chunks back in
        for key in std::iter::once(start).filter(|s| *s > 0) {
            div {
                key: "{key}",
                style: "height: {spacer_px}px;",
                onvisible: on_spacer_visible,
            }
        }
        div {
            id: GRID_ID,
            class: grid_class,
            onresize: on_resize,
            for search_resp in list.iter().skip(start) {
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
