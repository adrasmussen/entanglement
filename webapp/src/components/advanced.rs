use std::{
    collections::{HashMap, HashSet},
    marker::PhantomData,
};

use dioxus::prelude::*;
use tracing::error;

use crate::{
    common::{
        colors::CollectionColor,
        query_ui::{add_clause, op_label},
        search::{fetch_all_uuids, literal_filter},
    },
    components::{
        modal::{MODAL_STACK, Modal},
        query_bar::{QueryChips, SearchState},
        search::CompactSearchBar,
    },
};
use api::{
    WebError,
    collection::*,
    media::MediaUuid,
    query::{FieldSpec, FieldType, Op, SearchClass, SearchQuery, field_spec},
    search::SearchRequest,
};

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

#[derive(Props)]
pub struct AdvancedSearchTabProps<C: SearchClass + 'static> {
    search: SearchState,
    #[props(default)]
    class: PhantomData<fn() -> C>,
}

impl<C: SearchClass + 'static> Clone for AdvancedSearchTabProps<C> {
    fn clone(&self) -> Self {
        Self {
            search: self.search,
            class: PhantomData,
        }
    }
}

impl<C: SearchClass + 'static> PartialEq for AdvancedSearchTabProps<C> {
    fn eq(&self, other: &Self) -> bool {
        self.search == other.search
    }
}

// a filter builder generated from the class's field metadata: pick a field, one of its
// operators and a value, and the clause is added to the search text
#[component]
pub fn AdvancedSearchTab<C: SearchClass + 'static>(props: AdvancedSearchTabProps<C>) -> Element {
    let search = props.search;

    let mut field = use_signal(|| C::FIELDS.first().map(|f| f.name).unwrap_or_default());
    let mut op = use_signal(|| C::FIELDS.first().map(|f| f.default_op()));
    let mut value = use_signal(String::new);
    let mut to = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);

    let (Some(spec), Some(current_op)) = (field_spec::<C>(field()), op()) else {
        return rsx! {};
    };

    let has_filters = !SearchQuery::parse::<C>(&(search.signal)()).query.is_empty();

    rsx! {
        div { class: "text-search-options",
            form {
                class: "filter-builder",
                onsubmit: move |_| {
                    // no prevent_default: in dioxus-web 0.6 forms never submit natively,
                    // and calling it on a submit event re-enables the native submission
                    match add_clause::<C>(&search.text(), spec.name, current_op, &value(), &to()) {
                        Ok(text) => {
                            search.commit(text);
                            value.set(String::new());
                            to.set(String::new());
                            error.set(None);
                        }
                        Err(message) => error.set(Some(message)),
                    }
                },
                div { class: "filter-builder-row",
                    div { class: "form-group",
                        label { class: "form-label", "Field" }
                        select {
                            class: "form-select",
                            "data-testid": "builder-field",
                            value: "{spec.name}",
                            onchange: move |evt| {
                                if let Some(f) = field_spec::<C>(&evt.value()) {
                                    field.set(f.name);
                                    op.set(Some(f.default_op()));
                                    value.set(String::new());
                                    to.set(String::new());
                                    error.set(None);
                                }
                            },
                            for f in C::FIELDS.iter() {
                                option {
                                    key: "{f.name}",
                                    value: "{f.name}",
                                    selected: f.name == spec.name,
                                    "{f.label}"
                                }
                            }
                        }
                    }
                    div { class: "form-group",
                        label { class: "form-label", "Matches" }
                        select {
                            class: "form-select",
                            "data-testid": "builder-op",
                            value: "{current_op:?}",
                            onchange: move |evt| {
                                let chosen = field_spec::<C>(field())
                                    .and_then(|f| {
                                        f.ops.iter().find(|o| format!("{o:?}") == evt.value()).copied()
                                    });
                                if chosen.is_some() {
                                    op.set(chosen);
                                }
                            },
                            for o in spec.ops.iter() {
                                option {
                                    key: "{o:?}",
                                    value: "{o:?}",
                                    selected: *o == current_op,
                                    "{op_label(*o)}"
                                }
                            }
                        }
                    }
                    div { class: "form-group",
                        label { class: "form-label",
                            if current_op == Op::Range {
                                "From"
                            } else {
                                "Value"
                            }
                        }
                        {builder_input(spec, value, "builder-value")}
                    }
                    if current_op == Op::Range {
                        div { class: "form-group",
                            label { class: "form-label", "To" }
                            {builder_input(spec, to, "builder-value-to")}
                        }
                    }
                    button {
                        class: "btn btn-primary",
                        r#type: "submit",
                        "data-testid": "builder-add",
                        "Add filter"
                    }
                }
                div {
                    class: "form-help",
                    style: "font-size: 0.875rem; color: var(--text-tertiary); margin-top: var(--space-2);",
                    "data-testid": "builder-help",
                    "{spec.help} (for example "
                    code { "{spec.example}" }
                    ")"
                }
                if let Some(message) = error() {
                    div { class: "filter-builder-error", "data-testid": "builder-error", "{message}" }
                }
            }

            div { style: "margin-top: var(--space-4);",
                label { class: "form-label", "Current filters" }
                div { class: "query-chips", "data-testid": "builder-chips",
                    if has_filters {
                        QueryChips::<C> { search }
                    } else {
                        span { style: "font-size: 0.875rem; color: var(--text-tertiary);",
                            "None. {C::FREE_TEXT_HELP}."
                        }
                    }
                }
            }
        }
    }
}

// the value input for a field: a select for an enum, a text input otherwise
fn builder_input(
    spec: &'static FieldSpec,
    mut value: Signal<String>,
    testid: &'static str,
) -> Element {
    match spec.ty {
        FieldType::Enum(allowed) => rsx! {
            select {
                class: "form-select",
                "data-testid": testid,
                value: "{value}",
                onchange: move |evt| value.set(evt.value()),
                option { value: "", selected: value().is_empty(), "Choose..." }
                for a in allowed.iter() {
                    option {
                        key: "{a}",
                        value: "{a}",
                        selected: value() == *a,
                        "{a}"
                    }
                }
            }
        },
        _ => rsx! {
            input {
                class: "form-input",
                "data-testid": testid,
                placeholder: if spec.ty == FieldType::Date { "2020-06" } else { "text" },
                value: "{value}",
                oninput: move |evt| value.set(evt.value()),
            }
        },
    }
}

#[derive(Clone, PartialEq)]
pub enum BulkEditMode {
    EditTags,
    AddToCollection,
    RmFromCollection(CollectionUuid),
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
        BulkEditMode::RmFromCollection(collection_uuid) => (
            "Remove from Collection",
            Modal::BulkRmFromCollectionModal(bulk_edit_signal(), collection_uuid),
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
    // the page's search request (paging options are ignored), used to find every match
    search_req: Callback<(), SearchRequest>,
    modes: Vec<BulkEditMode>,
}

#[component]
pub fn BulkEditTab(props: BulkEditTabProps) -> Element {
    let mut bulk_edit_signal = props.bulk_edit_signal;
    let search_req = props.search_req;
    let modes = props.modes;
    let mut inverting = use_signal(|| false);
    let mut invert_error = use_signal(|| None::<String>);

    rsx! {
        div { class: "bulk-edit-options",
            div { class: "form-group",
                label { class: "form-label", "Bulk Edit" }
                p { style: "margin-bottom: var(--space-4); color: var(--text-secondary); font-size: 0.875rem;",
                    "Select multiple media items and apply changes to all of them at once. Enable bulk edit mode, then click items in the gallery to select them."
                }
            }

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
                    disabled: inverting(),
                    onclick: move |_| {
                        let selected = bulk_edit_uuids.clone();
                        let req = search_req.call(());
                        async move {
                            inverting.set(true);
                            invert_error.set(None);
                            match fetch_all_uuids(req).await {
                                Ok(all) => {
                                    bulk_edit_signal
                                        .set(Some(all.difference(&selected).copied().collect()));
                                }
                                Err(err) => {
                                    error!("failed to fetch all search results: {err}");
                                    invert_error.set(Some(err.to_string()));
                                }
                            }
                            inverting.set(false);
                        }
                    },
                    if inverting() {
                        "Inverting..."
                    } else {
                        "Invert Selection"
                    }
                }
                button {
                    class: "btn btn-secondary",
                    onclick: move |_| {
                        bulk_edit_signal.set(Some(HashSet::new()));
                    },
                    "Clear Selection"
                }
            }
            if let Some(err) = invert_error() {
                p { style: "color: var(--error); margin-top: var(--space-2);",
                    "Could not invert selection: {err}"
                }
            }
            div {
                class: "bulk-actions",
                style: "display: flex; gap: var(--space-3); margin-top: var(--space-4);",


                for mode in modes {
                    BulkEditButton { bulk_edit_signal, mode }
                }
            }
            div { class: "info-box", style: "margin-top: var(--space-4);",
                p { "Click on media items in the gallery to select them. Click again to deselect." }
                p { "Selected items will be highlighted with a blue border." }
            }
        } else {
            div { class: "info-box", style: "margin-top: var(--space-4);",
                p { "Enable bulk edit mode to modify many media at the same time." }
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
        let filter = literal_filter(&collection_search_signal());

        search_collections(&SearchCollectionsReq {
            filter,
            query: None,
            ..Default::default()
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
                h4 { class: "tab-section-title", "Add Collection Color" }

                CompactSearchBar {
                    search_signal: collection_search_signal,
                    placeholder: "Search collections to add color...",
                }

                if let Some(collections) = available_collections {
                    if collections.is_empty() {
                        div { class: "info-box", style: "text-align: center;",
                            p { "No more collections available to color. All matching collections are already colored." }
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
                h4 { class: "tab-section-title", "Colored Collections" }
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
            div {
                form {
                    style: "display: flex; align-items: center; gap: var(--space-2);",
                    onsubmit: move |event| {
                        if let Some(color) = event
                            .values()
                            .get(&format!("color-select-{collection_uuid}"))
                            .map(|v| CollectionColor::from(v.as_value()))
                        {
                            collection_color_signal.with_mut(|map| map.insert(collection_uuid, color));
                        }
                    },
                    select {
                        class: "form-select",
                        style: "min-width: 100px;",
                        name: "color-select-{collection_uuid}",
                        for available_color in CollectionColor::all() {
                            option {
                                value: "{available_color}",
                                selected: available_color == CollectionColor::Blue, // Default to blue
                                "{available_color}"
                            }
                        }
                    }
                    button { class: "btn btn-sm btn-primary", r#type: "submit", "Add Color" }
                }
            }
        }
    }
}
