use std::collections::{HashMap, HashSet};

use dioxus::prelude::*;
use tracing::error;
use web_sys::wasm_bindgen::JsCast;

use crate::{
    common::colors::CollectionColor,
    components::{
        modal::{MODAL_STACK, Modal},
        search::CompactSearchBar,
    },
};
use api::{WebError, collection::*, media::MediaUuid, search::*};

// TODO -- use show_signal for consistent cleanup... or move to a button?
#[derive(Clone, PartialEq, Props)]
pub struct AdvancedTabsProps {
    show_signal: Signal<bool>,
    tabs: HashMap<String, Element>,
}

#[component]
pub fn AdvancedTabs(props: AdvancedTabsProps) -> Element {
    let show_signal = props.show_signal;
    let tabs = props.tabs;

    if !show_signal() {
        return rsx! {};
    }

    let mut tab_names = tabs.keys().cloned().collect::<Vec<String>>();

    tab_names.sort();

    let default_tab = match tab_names.first() {
        Some(v) => v.clone(),
        None => return rsx! {},
    };

    let mut tab_signal = use_signal(|| default_tab);

    rsx! {
        div {
            class: "advanced-search-options",
            style: "margin-top: -16px; margin-bottom: var(--space-6); padding: var(--space-4); background-color: var(--neutral-50); border-radius: 0 0 var(--radius-lg) var(--radius-lg); box-shadow: var(--shadow-sm); border-top: 1px solid var(--neutral-200); animation: slide-down 0.2s ease-out;",
            h3 { style: "margin-bottom: var(--space-3); font-size: 1rem;", "Advanced Options" }

            div {
                class: "tabs-navigation",
                style: "display: flex; border-bottom: 1px solid var(--neutral-200); margin-bottom: var(--space-4);",

                for name in tab_names {

                    button {
                        class: if *tab_signal.read() == *name { "tab-button active" } else { "tab-button" },
                        style: "padding: var(--space-2) var(--space-4); background: none; border: none; border-bottom: 3px solid transparent; cursor: pointer;font-weight: 500; color: var(--text-secondary); transition: all var(--transition-fast) var(--easing-standard); margin-right: var(--space-2);"
                            .to_string()
                            + if *tab_signal.read() == *name {
                                "color: var(--primary); border-bottom-color: var(--primary);"
                            } else {
                                ""
                            },
                        onclick: move |_| tab_signal.set(name.clone()),

                        "{name}"
                    }
                }
            }

            ErrorBoundary {
                handle_error: |error: ErrorContext| {
                    rsx! {
                        if let Some(error_ui) = error.show() {
                            {error_ui}
                        } else {
                            div { "Tab encountered an error.  Check the logs or reach out the the administrators." }
                        }
                    }
                },

                {
                    tabs.get(&*tab_signal.read())
                        .ok_or_else(|| WebError::msg("unknown tab".to_owned()))?
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct AdvancedSearchTabProps {
    media_search_signal: Signal<String>,
}

#[component]
pub fn AdvancedSearchTab(props: AdvancedSearchTabProps) -> Element {
    let mut media_search_signal = props.media_search_signal;
    rsx! {
        div { class: "text-search-options",
            div { class: "form-group",
                label { class: "form-label", "Search Terms" }
                input {
                    class: "form-input",
                    placeholder: "Enter keywords separated by spaces...",
                    value: "{media_search_signal()}",
                    oninput: move |evt| media_search_signal.set(evt.value().clone()),
                }
                div {
                    class: "form-help",
                    style: "font-size: 0.875rem; color: var(--text-tertiary); margin-top: var(--space-1);",
                    "Search in file names, descriptions, and tags"
                }
            }
            div { style: "display: flex; gap: var(--space-4); margin-top: var(--space-4);",
                div { class: "form-group", style: "flex: 1;",
                    label { class: "form-label", "Search Mode" }
                    select { class: "form-select",
                        option { value: "all", selected: true, "Match All Words" }
                        option { value: "any", "Match Any Word" }
                        option { value: "exact", "Exact Phrase" }
                    }
                }
                div { class: "form-group", style: "flex: 1;",
                    label { class: "form-label", "Case Sensitive" }
                    div { style: "display: flex; align-items: center; height: 38px;",
                        input {
                            r#type: "checkbox",
                            id: "case-sensitive",
                            style: "margin-right: var(--space-2);",
                        }
                        label { r#for: "case-sensitive", "Enable case sensitivity" }
                    }
                }
            }
        }
    }
}

#[derive(Clone, PartialEq)]
pub enum BulkEditMode {
    EditTags,
    AddToCollection,
    //RmFromCollection,
    //Hide,
}
#[derive(Clone, PartialEq, Props)]
struct BulkEditButtonProps {
    mode: BulkEditMode,
    bulk_edit_signal: Signal<Option<HashSet<MediaUuid>>>,
}

#[component]
fn BulkEditButton(props: BulkEditButtonProps) -> Element {
    let bulk_edit_signal = props.bulk_edit_signal;

    let (text, target) = match props.mode {
        BulkEditMode::EditTags => ("Edit Tags", Modal::BulkEditTags(bulk_edit_signal())),
        BulkEditMode::AddToCollection => (
            "Add to Collection",
            Modal::BulkAddToCollection(bulk_edit_signal()),
        ),
    };

    rsx! {
        button {
            class: "btn btn-primary",
            onclick: move |_| {
                MODAL_STACK.with_mut(|v| { v.push(target.clone()) });
            },
            "{text}"
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct BulkEditTabProps {
    bulk_edit_signal: Signal<Option<HashSet<MediaUuid>>>,
    media_uuids: Memo<Option<HashSet<MediaUuid>>>,
    modes: Vec<BulkEditMode>,
}

#[component]
pub fn BulkEditTab(props: BulkEditTabProps) -> Element {
    let mut bulk_edit_signal = props.bulk_edit_signal;
    let media_uuids = props.media_uuids;
    let modes = props.modes;

    rsx! {
        div { class: "bulk-edit-options",
            div {
                class: "bulk-edit-controls",
                style: "margin-bottom: var(--space-4);",
                div { class: "form-group",
                    div { style: "display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-4);",
                        {
                            match bulk_edit_signal() {
                                None => {
                                    rsx! {
                                        button {
                                            class: "btn btn-primary",
                                            onclick: move |_| {
                                                bulk_edit_signal.set(Some(HashSet::new()));
                                            },
                                            "Enable Bulk Edit Mode"
                                        }
                                    }
                                }
                                Some(_) => {
                                    rsx! {
                                        button {
                                            class: "btn btn-danger",
                                            onclick: move |_| {
                                                bulk_edit_signal.set(None);
                                            },
                                            "Disable Bulk Edit Mode"
                                        }
                                    }
                                }
                            }
                        }

                        if let Some(media_uuids) = bulk_edit_signal() {
                            span { style: "color: var(--text-secondary);",
                                "{media_uuids.len()} items selected"
                            }
                        }
                    }
                }
            }
        }

        if let Some(bulk_edit_uuids) = bulk_edit_signal() {
            div {
                class: "bulk-actions",
                style: "display: flex; gap: var(--space-3); margin-top: var(--space-4);",
                button {
                    class: "btn btn-secondary",
                    onclick: move |_| {
                        if let Some(media_uuids) = media_uuids() {
                            bulk_edit_signal
                                .set(Some(media_uuids.difference(&bulk_edit_uuids).copied().collect()));
                        }
                    },
                    "Invert Selection"
                }
                button {
                    class: "btn btn-secondary",
                    onclick: move |_| {
                        bulk_edit_signal.set(Some(HashSet::new()));
                    },
                    "Clear Selection"
                }
            }
            div {
                class: "bulk-actions",
                style: "display: flex; gap: var(--space-3); margin-top: var(--space-4);",


                for mode in modes {
                    BulkEditButton { bulk_edit_signal, mode }
                }
            }
            div {
                class: "bulk-edit-instructions",
                style: "margin-top: var(--space-4); padding: var(--space-3); background-color: var(--neutral-50); border-radius: var(--radius-md);",
                p { style: "margin: 0; color: var(--text-secondary);",
                    "Click on media items in the gallery to select them. Click again to deselect."
                }
                p { style: "margin-top: var(--space-2); color: var(--text-tertiary);",
                    "Selected items will be highlighted with a blue border."
                }
            }
        } else {
            div {
                class: "bulk-edit-disabled",
                style: "margin-top: var(--space-4); padding: var(--space-3); background-color: var(--neutral-50); border-radius: var(--radius-md);",
                p { style: "margin: 0; color: var(--text-secondary);",
                    "Enable bulk edit mode to modify many media at the same time."
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct CollectionColorTabProps {
    collection_color_signal: Signal<HashMap<CollectionUuid, CollectionColor>>,
}

#[component]
pub fn CollectionColorTab(props: CollectionColorTabProps) -> Element {
    let collection_color_signal = props.collection_color_signal;
    let collection_search_signal = use_signal(String::new);

    // Track which collections are already selected for coloring
    let selected_collections = use_memo(move || {
        collection_color_signal()
            .keys()
            .cloned()
            .collect::<Vec<_>>()
    });

    let collections_future = use_resource(move || async move {
        let filter = collection_search_signal()
            .split_whitespace()
            .map(|s| s.to_owned())
            .collect();

        search_collections(&SearchCollectionsReq {
            filter: SearchFilter::SubstringAny { filter },
        })
        .await
    });

    // Get available collections (excluding already selected ones)
    let available_collections = match &*collections_future.read() {
        Some(Ok(response)) => Some(
            response
                .collections
                .iter()
                .filter(|uuid| !selected_collections().contains(uuid))
                .cloned()
                .collect::<Vec<_>>(),
        ),
        Some(Err(_)) => None,
        None => None,
    };

    rsx! {
        div { class: "collection-color-options",
            div { class: "form-group",
                label { class: "form-label", "Collection Colors" }
                p { style: "margin-bottom: var(--space-4); color: var(--text-secondary); font-size: 0.875rem;",
                    "Assign colors to collections to highlight media cards. Media belonging to colored collections will display with those colors."
                }
            }

            // Currently colored collections
            ColoredCollectionList { collection_color_signal }

            // Add new collection color section
            div { class: "add-collection-color",
                h4 { style: "margin-bottom: var(--space-3); font-size: 1rem; color: var(--text-primary);",
                    "Add Collection Color"
                }

                CompactSearchBar {
                    search_signal: collection_search_signal,
                    placeholder: "Search collections to add color...",
                }

                if let Some(collections) = available_collections {
                    if collections.is_empty() {
                        div {
                            class: "empty-state",
                            style: "padding: var(--space-4); text-align: center; color: var(--text-tertiary); background-color: var(--neutral-50); border-radius: var(--radius-md);",
                            "No more collections available to color. All matching collections are already colored."
                        }
                    } else {
                        div {
                            class: "available-collections",
                            style: "max-height: 200px; overflow-y: auto; border: 1px solid var(--border); border-radius: var(--radius-md);",
                            for collection_uuid in collections {
                                AvailableCollectionItem {
                                    key: "{collection_uuid}",
                                    collection_uuid,
                                    collection_color_signal,
                                }
                            }
                        }
                    }
                } else {
                    div { style: "display: flex; flex-direction: column; gap: var(--space-2);",
                        for _ in 0..3 {
                            div {
                                class: "skeleton",
                                style: "height: 60px; border-radius: var(--radius-md);",
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
struct ColoredCollectionListProps {
    collection_color_signal: Signal<HashMap<CollectionUuid, CollectionColor>>,
}

#[component]
fn ColoredCollectionList(props: ColoredCollectionListProps) -> Element {
    let collection_color_signal = props.collection_color_signal;

    if !collection_color_signal().is_empty() {
        rsx! {
            div {
                class: "colored-collections-list",
                style: "margin-bottom: var(--space-6);",
                h4 { style: "margin-bottom: var(--space-3); font-size: 1rem; color: var(--text-primary);",
                    "Colored Collections"
                }
                div { style: "display: flex; flex-direction: column; gap: var(--space-3);",
                    for (collection_uuid , color) in collection_color_signal().iter() {
                        ColoredCollectionItem {
                            key: "{collection_uuid}",
                            collection_uuid: *collection_uuid,
                            color: *color,
                            collection_color_signal,
                        }
                    }
                }
            }
        }
    } else {
        rsx! {}
    }
}

#[derive(Clone, PartialEq, Props)]
struct ColoredCollectionItemProps {
    collection_uuid: CollectionUuid,
    color: CollectionColor,
    collection_color_signal: Signal<HashMap<CollectionUuid, CollectionColor>>,
}

#[component]
fn ColoredCollectionItem(props: ColoredCollectionItemProps) -> Element {
    let collection_uuid = props.collection_uuid;
    let color = props.color;
    let mut collection_color_signal = props.collection_color_signal;

    // Fetch collection details
    let collection_future =
        use_resource(
            move || async move { get_collection(&GetCollectionReq { collection_uuid }).await },
        );

    let collection = &*collection_future.read();

    match collection {
        Some(Ok(result)) => {
            let collection = result.collection.clone();
            rsx! {
                div {
                    class: "colored-collection-item",
                    style: format!(
                        "display: flex; align-items: center; padding: var(--space-3); border: 1px solid var(--border); border-radius: var(--radius-md); background-color: {};",
                        color.to_light_css_color(),
                    ),

                    div {
                        style: format!(
                            "width: 24px; height: 24px; border-radius: 50%; background-color: {}; margin-right: var(--space-3); border: 2px solid white; box-shadow: 0 1px 3px rgba(0,0,0,0.2);",
                            color.to_css_color(),
                        ),
                    }

                    div { style: "flex: 1; margin-right: var(--space-3);",
                        div { style: "font-weight: 500; color: var(--text-primary);",
                            "{collection.name}"
                        }
                        div { style: "font-size: 0.875rem; color: var(--text-secondary);",
                            "Group: {collection.gid}"
                        }
                    }

                    select {
                        class: "form-select",
                        style: "margin-right: var(--space-2); max-width: 100px;",
                        value: "{color}",
                        onchange: move |evt| {
                            match evt.value().parse::<String>() {
                                Ok(color) => {
                                    if CollectionColor::all().contains(&color.clone().into()) {
                                        collection_color_signal
                                            .with_mut(|map| {
                                                map.insert(collection_uuid, color.into());
                                            });
                                    }
                                }
                                _ => error!("internal error -- invalid color"),
                            }
                        },
                        for available_color in CollectionColor::all() {
                            option {
                                value: "{available_color}",
                                selected: available_color == color,
                                "{available_color}"
                            }
                        }
                    }

                    button {
                        class: "btn btn-sm btn-danger",
                        onclick: move |_| {
                            collection_color_signal
                                .with_mut(|map| {
                                    map.remove(&collection_uuid);
                                });
                        },
                        "Remove"
                    }
                }
            }
        }

        _ => {
            error!("failed to get collection while coloring");
            rsx! {
                div {
                    div {
                        class: "skeleton",
                        style: "height: 18px; width: 120px; margin-bottom: 4px;",
                    }
                    div {
                        class: "skeleton",
                        style: "height: 14px; width: 80px;",
                    }
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
struct AvailableCollectionItemProps {
    collection_uuid: CollectionUuid,
    collection_color_signal: Signal<HashMap<CollectionUuid, CollectionColor>>,
}

#[component]
fn AvailableCollectionItem(props: AvailableCollectionItemProps) -> Element {
    let collection_uuid = props.collection_uuid;
    let mut collection_color_signal = props.collection_color_signal;

    // Fetch collection details
    let collection_future =
        use_resource(
            move || async move { get_collection(&GetCollectionReq { collection_uuid }).await },
        );

    let collection_data = match &*collection_future.read() {
        Some(Ok(response)) => Some(response.collection.clone()),
        _ => None,
    };

    rsx! {
        div {
            class: "available-collection-item",
            style: "display: flex; align-items: center; justify-content: space-between; padding: var(--space-3); border-bottom: 1px solid var(--border); transition: background-color var(--transition-fast) var(--easing-standard);",

            // Collection info
            div { style: "flex: 1;",
                if let Some(collection) = collection_data {
                    div {
                        div { style: "font-weight: 500; color: var(--text-primary);",
                            "{collection.name}"
                        }
                        div { style: "font-size: 0.875rem; color: var(--text-secondary);",
                            "Group: {collection.gid}"
                            if !collection.note.is_empty() {
                                " • {collection.note}"
                            }
                        }
                    }
                } else {
                    div {
                        div {
                            class: "skeleton",
                            style: "height: 18px; width: 120px; margin-bottom: 4px;",
                        }
                        div {
                            class: "skeleton",
                            style: "height: 14px; width: 200px;",
                        }
                    }
                }
            }

            // Color selector and add button
            div { style: "display: flex; align-items: center; gap: var(--space-2);",
                select {
                    class: "form-select",
                    style: "min-width: 100px;",
                    id: "color-select-{collection_uuid}",
                    for available_color in CollectionColor::all() {
                        option {
                            value: "{available_color}",
                            selected: available_color == CollectionColor::Blue, // Default to blue
                            "{available_color}"
                        }
                    }
                }
                button {
                    class: "btn btn-sm btn-primary",
                    onclick: move |_| {
                        if let Some(window) = web_sys::window() {
                            if let Some(document) = window.document() {
                                if let Some(select) = document
                                    .get_element_by_id(&format!("color-select-{}", collection_uuid))
                                {
                                    if let Some(select) = select.dyn_ref::<web_sys::HtmlSelectElement>()
                                    {
                                        let selected_value = select.value();
                                        for available_color in CollectionColor::all() {
                                            if available_color.to_string() == selected_value {
                                                collection_color_signal
                                                    .with_mut(|map| {
                                                        map.insert(collection_uuid, available_color);
                                                    });
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
                    "Add Color"
                }
            }
        }
    }
}
