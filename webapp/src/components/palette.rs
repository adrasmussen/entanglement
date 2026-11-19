use dioxus::prelude::*;
use dioxus_router::prelude::navigator;
use gloo_timers::future::TimeoutFuture;
use tracing::warn;

use crate::{
    common::palette::{PaletteRow, first_active, move_active, rows},
    components::modal::MODAL_STACK,
};
use api::{
    omni::{OmniSearchReq, OmniSearchResp, omni_search},
    thumbnail_link,
};

// the ⌘K search palette: one box that searches every class at once (OmniSearch) and shows
// grouped results.  picking a hit opens it; "See all" opens the class's page with the same
// search prefilled through ?q=.  the row logic lives in common/palette.rs

pub static PALETTE_OPEN: GlobalSignal<bool> = Signal::global(|| false);

// how long typing must settle before a search goes out
const PALETTE_DEBOUNCE_MS: u32 = 150;
const PER_GROUP: u32 = 5;

// ⌘K / Ctrl+K anywhere: the listener only reports the key, so the Rust side decides (a modal
// being open, toggling).  capture phase, so inputs that handle keys themselves don't eat it
const SHORTCUT_JS: &str = r#"
    if (window.__paletteKeyOff) window.__paletteKeyOff();
    const onKey = (e) => {
        if ((e.metaKey || e.ctrlKey) && !e.altKey && !e.shiftKey && e.key.toLowerCase() === "k") {
            e.preventDefault();
            dioxus.send(true);
        }
    };
    window.addEventListener("keydown", onKey, true);
    window.__paletteKeyOff = () => {
        window.removeEventListener("keydown", onKey, true);
        delete window.__paletteKeyOff;
    };
"#;

// the palette's shortcut listener and, while open, the palette itself; mounted once in the
// NavBar layout so it is on every page
#[component]
pub fn SearchPalette() -> Element {
    use_hook(|| {
        let mut listener = document::eval(SHORTCUT_JS);
        spawn(async move {
            loop {
                match listener.recv::<bool>().await {
                    Ok(_) => {
                        // a modal owns the keyboard while it's open
                        if MODAL_STACK.peek().is_empty() {
                            let open = *PALETTE_OPEN.peek();
                            *PALETTE_OPEN.write() = !open;
                        }
                    }
                    Err(err) => {
                        warn!("search palette shortcut stopped: {err:?}");
                        break;
                    }
                }
            }
        });
    });
    use_drop(|| {
        let _ = document::eval("if (window.__paletteKeyOff) window.__paletteKeyOff();");
    });

    rsx! {
        if PALETTE_OPEN() {
            PalettePanel {}
        }
    }
}

fn close() {
    *PALETTE_OPEN.write() = false;
}

// the shortcut as the platform writes it
const IS_MAC_JS: &str = r#"return /mac|iphone|ipad/i.test(navigator.userAgentData?.platform || navigator.platform || "");"#;

pub fn shortcut_label(mac: bool) -> &'static str {
    if mac { "\u{2318}K" } else { "Ctrl+K" }
}

// the header's search box: a button that looks like one and opens the palette, so the palette's
// input stays the only place to type.  on a phone it shrinks to the magnifier
#[component]
pub fn HeaderSearch() -> Element {
    let mut mac = use_signal(|| true);
    use_hook(|| {
        spawn(async move {
            if let Ok(v) = document::eval(IS_MAC_JS).await {
                mac.set(v.as_bool().unwrap_or(true));
            }
        });
    });
    let keys = shortcut_label(mac());

    rsx! {
        button {
            class: "header-search",
            r#type: "button",
            "data-testid": "header-search",
            "aria-label": "Search everything ({keys})",
            "aria-keyshortcuts": "Meta+K Control+K",
            "aria-haspopup": "dialog",
            onclick: move |_| *PALETTE_OPEN.write() = true,
            svg {
                class: "header-search-icon",
                view_box: "0 0 24 24",
                width: "16",
                height: "16",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "2",
                stroke_linecap: "round",
                "aria-hidden": "true",
                circle { cx: "11", cy: "11", r: "7" }
                line { x1: "16.5", y1: "16.5", x2: "21", y2: "21" }
            }
            span { class: "header-search-text", "{keys} to search" }
        }
    }
}

#[component]
fn PalettePanel() -> Element {
    let mut text = use_signal(String::new);
    let mut active = use_signal(|| None::<usize>);
    let nav = navigator();

    // a new text restarts the resource, which drops the pending future, so the pause before
    // the request debounces typing.  the answer carries the text it was for
    let results = use_resource(move || async move {
        let query = text();
        if query.trim().is_empty() {
            return None;
        }
        TimeoutFuture::new(PALETTE_DEBOUNCE_MS).await;
        let resp = omni_search(&OmniSearchReq {
            query: query.clone(),
            kinds: Vec::new(),
            per_group: Some(PER_GROUP),
        })
        .await;
        Some((query, resp.map_err(|e| e.to_string())))
    });

    // only results for exactly what is typed are shown, so stale ones never flash
    let current: Option<(OmniSearchResp, Vec<PaletteRow>)> = match &*results.read() {
        Some(Some((q, Ok(resp)))) if *q == text() => Some((resp.clone(), rows(resp, q))),
        _ => None,
    };
    let error: Option<String> = match &*results.read() {
        Some(Some((q, Err(e)))) if *q == text() => Some(e.clone()),
        _ => None,
    };
    let searching = !text().trim().is_empty() && current.is_none() && error.is_none();

    // the first selectable row becomes active whenever a new set of rows arrives
    let row_list: Vec<PaletteRow> = current.as_ref().map(|(_, r)| r.clone()).unwrap_or_default();
    let first = first_active(&row_list);
    use_effect(use_reactive!(|first| active.set(first)));

    let open_row = move |row: &PaletteRow| {
        if let Some(route) = row.route() {
            close();
            nav.push(route);
        }
    };

    let rows_for_keys = row_list.clone();
    let onkeydown = move |evt: KeyboardEvent| match evt.key() {
        Key::ArrowDown | Key::ArrowUp => {
            evt.prevent_default();
            let delta = if evt.key() == Key::ArrowDown { 1 } else { -1 };
            let next = move_active(&rows_for_keys, active(), delta);
            active.set(next);
            if let Some(i) = next {
                let _ = document::eval(&format!(
                    "const r = document.getElementById('palette-row-{i}'); if (r) r.scrollIntoView({{block: 'nearest'}});"
                ));
            }
        }
        Key::Enter => {
            evt.prevent_default();
            if let Some(row) = active().and_then(|i| rows_for_keys.get(i)) {
                open_row(row);
            }
        }
        Key::Escape => {
            evt.prevent_default();
            evt.stop_propagation();
            close();
        }
        _ => {}
    };

    let active_id = active()
        .map(|i| format!("palette-row-{i}"))
        .unwrap_or_default();

    rsx! {
        div {
            class: "palette-overlay",
            "data-testid": "palette",
            onclick: move |_| close(),
            div {
                class: "palette-panel",
                role: "dialog",
                "aria-modal": "true",
                "aria-label": "Search everything",
                onclick: move |evt| evt.stop_propagation(),
                input {
                    class: "palette-input",
                    "data-testid": "palette-input",
                    r#type: "text",
                    placeholder: "Search media, collections and libraries...",
                    autocomplete: "off",
                    spellcheck: "false",
                    maxlength: "{api::query::MAX_FILTER_CHARS}",
                    role: "combobox",
                    "aria-expanded": "{!row_list.is_empty()}",
                    "aria-controls": "palette-list",
                    "aria-activedescendant": "{active_id}",
                    value: "{text}",
                    oninput: move |evt| text.set(evt.value()),
                    onkeydown,
                    onmounted: move |evt| async move {
                        let _ = evt.set_focus(true).await;
                    },
                }
                div { class: "palette-hint",
                    "Words, or fields like "
                    code { "tag:beach" }
                    " "
                    code { "name:IMG_" }
                    " "
                    code { "path:photos" }
                    span { class: "palette-keys", "\u{2191}\u{2193} to move \u{b7} Enter to open \u{b7} Esc to close" }
                }
                div {
                    class: "palette-list",
                    id: "palette-list",
                    role: "listbox",
                    "data-testid": "palette-list",
                    if let Some(message) = error {
                        div { class: "palette-note", "data-testid": "palette-note", "{message}" }
                    } else if searching {
                        div { class: "palette-note", "Searching..." }
                    } else if let Some((resp, list)) = current {
                        if list.is_empty() {
                            div { class: "palette-note", "data-testid": "palette-empty", "Nothing matches" }
                        }
                        for (i , row) in list.iter().enumerate() {
                            match row {
                                PaletteRow::Heading { text } => rsx! {
                                    div {
                                        key: "{i}",
                                        class: "palette-heading",
                                        "data-testid": "palette-heading",
                                        "{text}"
                                    }
                                },
                                PaletteRow::Note { text } => rsx! {
                                    div {
                                        key: "{i}",
                                        class: "palette-note",
                                        "data-testid": "palette-note",
                                        "{text}"
                                    }
                                },
                                PaletteRow::Hit { group, index, .. } => {
                                    let hit = resp.groups[*group].hits[*index].clone();
                                    let row = row.clone();
                                    rsx! {
                                        div {
                                            key: "{i}",
                                            id: "palette-row-{i}",
                                            class: if active() == Some(i) { "palette-row active" } else { "palette-row" },
                                            role: "option",
                                            "aria-selected": "{active() == Some(i)}",
                                            "data-testid": "palette-hit",
                                            "data-href": "{row.route().map(|r| r.to_string()).unwrap_or_default()}",
                                            onmouseenter: move |_| active.set(Some(i)),
                                            onclick: move |_| open_row(&row),
                                            PaletteThumb { key: "{hit.title}-{i}", media: hit.thumbnail }
                                            div { class: "palette-text",
                                                div { class: "palette-title", "data-testid": "palette-title", "{hit.title}" }
                                                if !hit.subtitle.is_empty() {
                                                    div { class: "palette-subtitle", "{hit.subtitle}" }
                                                }
                                            }
                                        }
                                    }
                                }
                                PaletteRow::SeeAll { text, .. } => {
                                    let label = text.clone();
                                    let row = row.clone();
                                    rsx! {
                                        div {
                                            key: "{i}",
                                            id: "palette-row-{i}",
                                            class: if active() == Some(i) { "palette-row palette-see-all active" } else { "palette-row palette-see-all" },
                                            role: "option",
                                            "aria-selected": "{active() == Some(i)}",
                                            "data-testid": "palette-see-all",
                                            "data-href": "{row.route().map(|r| r.to_string()).unwrap_or_default()}",
                                            onmouseenter: move |_| active.set(Some(i)),
                                            onclick: move |_| open_row(&row),
                                            "{label} \u{2192}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// a hit's thumbnail; an empty tile when there is none or it fails to load (records without
// a file, as the grid's "No preview" tiles do), so the rows never show a broken image
#[component]
fn PaletteThumb(media: Option<api::media::MediaUuid>) -> Element {
    let mut failed = use_signal(|| false);
    match media {
        Some(m) if !failed() => rsx! {
            img {
                class: "palette-thumb",
                src: "{thumbnail_link(m)}",
                alt: "",
                loading: "lazy",
                onerror: move |_| failed.set(true),
            }
        },
        _ => rsx! {
            div { class: "palette-thumb palette-thumb-empty" }
        },
    }
}
