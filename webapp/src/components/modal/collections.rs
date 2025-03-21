use std::collections::HashSet;

use dioxus::prelude::*;
use gloo_timers::callback::Timeout;

use crate::components::modal::{ModalSize, ModernModal, MODAL_STACK};
use api::{auth::*, collection::*, media::MediaUuid, unfold_set, FOLDING_SEPARATOR};

#[derive(Clone, PartialEq, Props)]
pub struct CreateCollectionModalProps {
    update_signal: Signal<()>,
}

#[component]
pub fn CreateCollectionModal(props: CreateCollectionModalProps) -> Element {
    let mut update_signal = props.update_signal;
    let mut status_message = use_signal(|| String::new());

    // Form state
    let mut collection_name = use_signal(|| String::new());
    let mut collection_group = use_signal(|| String::new());
    let mut collection_note = use_signal(|| String::new());
    let mut collection_tags = use_signal(|| String::new());

    // Form validation state
    let mut name_error = use_signal(|| String::new());
    let mut group_error = use_signal(|| String::new());

    // Display the users of the given group
    let group_future = use_resource(move || async move {
        let gid = collection_group();

        if gid.trim().is_empty() {
            return HashSet::new();
        }

        match get_users_in_group(&GetUsersInGroupReq { gid }).await {
            Ok(resp) => return resp.uids,
            Err(err) => {
                group_error.set(err.to_string());
                return HashSet::new();
            }
        }
    });

    // Handle submission
    let handle_submit = move |_| async move {
        // Reset validation errors
        name_error.set(String::new());
        group_error.set(String::new());

        // Basic validation
        let mut is_valid = true;

        if collection_name().trim().is_empty() {
            name_error.set("Collection name is required".into());
            is_valid = false;
        }

        if collection_group().trim().is_empty() {
            group_error.set("Group ID is required".into());
            is_valid = false;
        }

        if !is_valid {
            return;
        }

        // We're ready to submit
        status_message.set("Creating collection...".into());

        match add_collection(&AddCollectionReq {
            collection: Collection {
                uid: "".to_string(),
                gid: collection_group(),
                mtime: 0,
                name: collection_name(),
                note: collection_note(),
                tags: unfold_set(&collection_tags()),
            },
        })
        .await
        {
            Ok(resp) => {
                status_message.set(format!(
                    "Collection created with ID: {}",
                    resp.collection_uuid
                ));
                update_signal.set(());

                // Close the modal after a short delay to show success message
                let task = Timeout::new(1500, move || {
                    MODAL_STACK.with_mut(|v| v.pop());
                });
                task.forget();
            }
            Err(err) => {
                status_message.set(format!("Error: {}", err));
            }
        }
    };

    let footer = rsx! {
        span { class: "status-message", style: "color: var(--primary);", "{status_message}" }
        div {
            class: "modal-buttons",
            style: "display: flex; gap: var(--space-4); justify-content: flex-end;",
            button {
                class: "btn btn-secondary",
                onclick: move |_| {
                    MODAL_STACK.with_mut(|v| v.pop());
                },
                "Cancel"
            }
            button { class: "btn btn-primary", onclick: handle_submit, "Create Collection" }
        }
    };

    // Check if we have group members to display
    let group_members = &*group_future.read();
    let has_members = match group_members {
        Some(members) => !members.is_empty(),
        None => false,
    };

    rsx! {
        ModernModal { title: "Create New Collection", size: ModalSize::Medium, footer,
            div { class: "create-collection-form",
                div { class: "form-group",
                    label { class: "form-label", "Collection Name" }
                    input {
                        class: "form-input",
                        r#type: "text",
                        value: "{collection_name}",
                        oninput: move |evt| collection_name.set(evt.value().clone()),
                        placeholder: "My Amazing Photos",
                    }
                    if !name_error().is_empty() {
                        div {
                            class: "form-error",
                            style: "color: var(--error); font-size: 0.875rem; margin-top: 0.25rem;",
                            "{name_error}"
                        }
                    }
                }

                div { class: "form-group",
                    label { class: "form-label", "Group ID" }
                    div { style: "display: flex; gap: var(--space-2);",
                        input {
                            class: "form-input",
                            r#type: "text",
                            value: "{collection_group}",
                            oninput: move |evt| collection_group.set(evt.value().clone()),
                            placeholder: "users",
                            style: "flex: 1;",
                        }
                    }
                    if !group_error().is_empty() {
                        div {
                            class: "form-error",
                            style: "color: var(--error); font-size: 0.875rem; margin-top: 0.25rem;",
                            "{group_error}"
                        }
                    }
                    div {
                        class: "form-help",
                        style: "color: var(--text-tertiary); font-size: 0.875rem; margin-top: 0.25rem;",
                        "Group ID determines who can access this collection"
                    }
                }

                // Group members display
                if has_members {
                    div {
                        class: "group-members-container",
                        style: "
                            margin-top: var(--space-3);
                            margin-bottom: var(--space-3);
                            padding: var(--space-3);
                            background-color: var(--neutral-50);
                            border-radius: var(--radius-md);
                            border: 1px solid var(--neutral-200);
                        ",
                        h4 { style: "
                                font-size: 0.875rem;
                                margin-bottom: var(--space-2);
                                color: var(--text-secondary);
                                display: flex;
                                align-items: center;
                                gap: var(--space-2);
                            ",
                            svg {
                                width: "16",
                                height: "16",
                                view_box: "0 0 24 24",
                                fill: "none",
                                stroke: "currentColor",
                                stroke_width: "2",
                                stroke_linecap: "round",
                                stroke_linejoin: "round",
                                "class": "feather feather-users",
                                path { d: "M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2" }
                                circle { cx: "9", cy: "7", r: "4" }
                                path { d: "M23 21v-2a4 4 0 0 0-3-3.87" }
                                path { d: "M16 3.13a4 4 0 0 1 0 7.75" }
                            }
                            "Group Members"
                        }
                        match group_members {
                            Some(members) => {
                                rsx! {
                                    div {
                                        class: "members-list",
                                        style: "
                                            display: flex;
                                            flex-wrap: wrap;
                                            gap: var(--space-2);
                                        ",
                                        for member in members.iter() {
                                            div {
                                                class: "member-badge",
                                                style: "
                                                    display: inline-flex;
                                                    align-items: center;
                                                    padding: var(--space-1) var(--space-2);
                                                    background-color: var(--primary-light);
                                                    color: white;
                                                    border-radius: var(--radius-full);
                                                    font-size: 0.75rem;
                                                ",
                                                "{member}"
                                            }
                                        }
                                    }
                                    div { style: "
                                            margin-top: var(--space-2);
                                            font-size: 0.75rem;
                                            color: var(--text-tertiary);
                                        ",
                                        "Total members: {members.len()}"
                                    }
                                }
                            }
                            None => {
                                rsx! {
                                    div { class: "skeleton", style: "height: 1.5rem; width: 100%;" }
                                }
                            }
                        }
                    }
                } else if !collection_group().is_empty() {
                    div {
                        class: "group-members-container",
                        style: "
                            margin-top: var(--space-3);
                            margin-bottom: var(--space-3);
                            padding: var(--space-3);
                            background-color: var(--neutral-50);
                            border-radius: var(--radius-md);
                            border: 1px solid var(--neutral-200);
                            color: var(--text-tertiary);
                            text-align: center;
                            font-style: italic;
                        ",
                        "No members found in this group"
                    }
                }

                div { class: "form-group",
                    label { class: "form-label", "Description (optional)" }
                    textarea {
                        class: "form-textarea",
                        rows: 3,
                        value: "{collection_note}",
                        oninput: move |evt| collection_note.set(evt.value().clone()),
                        placeholder: "Add a description for this collection...",
                    }
                }
                div { class: "form-group",
                    label { class: "form-label", "Tags (optional)" }
                    textarea {
                        class: "form-textarea",
                        rows: 3,
                        value: "{collection_tags}",
                        oninput: move |evt| collection_tags.set(evt.value().clone()),
                        placeholder: format!("Add tags for this collection, separated by {}", FOLDING_SEPARATOR),
                    }
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct EditCollectionModalProps {
    update_signal: Signal<()>,
    collection_uuid: CollectionUuid,
}

#[component]
pub fn EditCollectionModal(props: EditCollectionModalProps) -> Element {
    let collection_uuid = props.collection_uuid;
    let mut update_signal = props.update_signal;
    let mut status_message = use_signal(|| String::new());

    // Form state
    let mut collection_name = use_signal(|| String::new());
    let mut collection_note = use_signal(|| String::new());

    // Fetch collection details to pre-fill the form
    let collection_future = use_resource(move || async move {
        get_collection(&GetCollectionReq {
            collection_uuid: collection_uuid,
        })
        .await
    });

    // Handle form initialization
    use_effect(move || {
        if let Some(Ok(result)) = &*collection_future.read() {
            collection_name.set(result.collection.name.clone());
            collection_note.set(result.collection.note.clone());
        }
    });

    // Form validation state
    let mut name_error = use_signal(|| String::new());

    // Handle submission
    let handle_submit = move |_| async move {
        // Reset validation errors
        name_error.set(String::new());

        // Basic validation
        let mut is_valid = true;

        if collection_name().trim().is_empty() {
            name_error.set("Collection name is required".into());
            is_valid = false;
        }

        if !is_valid {
            return;
        }

        // We're ready to submit
        status_message.set("Updating collection...".into());

        match update_collection(&UpdateCollectionReq {
            collection_uuid,
            update: CollectionUpdate {
                name: Some(collection_name()),
                note: Some(collection_note()),
                tags: None,
            },
        })
        .await
        {
            Ok(_) => {
                status_message.set("Collection updated successfully".into());
                update_signal.set(());

                // Close the modal after a short delay to show success message
                let task = Timeout::new(1500, move || {
                    MODAL_STACK.with_mut(|v| v.pop());
                });
                task.forget();
            }
            Err(err) => {
                status_message.set(format!("Error: {}", err));
            }
        }
    };

    let footer = rsx! {
        span { class: "status-message", style: "color: var(--primary);", "{status_message}" }
        div {
            class: "modal-buttons",
            style: "display: flex; gap: var(--space-4); justify-content: flex-end;",
            button {
                class: "btn btn-secondary",
                onclick: move |_| {
                    MODAL_STACK.with_mut(|v| v.pop());
                },
                "Cancel"
            }
            button { class: "btn btn-primary", onclick: handle_submit, "Save Changes" }
        }
    };

    rsx! {
        ModernModal { title: "Edit Collection", size: ModalSize::Medium, footer,
            div { class: "edit-collection-form",
                match &*collection_future.read() {
                    Some(Ok(_)) => {
                        rsx! {
                            div { class: "form-group",
                                label { class: "form-label", "Collection Name" }
                                input {
                                    class: "form-input",
                                    r#type: "text",
                                    value: "{collection_name}",
                                    oninput: move |evt| collection_name.set(evt.value().clone()),
                                    placeholder: "My Amazing Photos",
                                }
                                if !name_error().is_empty() {
                                    div {
                                        class: "form-error",
                                        style: "color: var(--error); font-size: 0.875rem; margin-top: 0.25rem;",
                                        "{name_error}"
                                    }
                                }
                            }
                            div { class: "form-group",
                                label { class: "form-label", "Description (optional)" }
                                textarea {
                                    class: "form-textarea",
                                    rows: 3,
                                    value: "{collection_note}",
                                    oninput: move |evt| collection_note.set(evt.value().clone()),
                                    placeholder: "Add a description for this collection...",
                                }
                            }
                        }
                    }
                    Some(Err(err)) => rsx! {
                        div {
                            class: "error-state",
                            style: "color: var(--error); padding: var(--space-4); text-align: center;",
                            "Error loading collection: {err}"
                        }
                    },
                    None => rsx! {
                        div { class: "loading-state",
                            // Loading spinner or skeleton UI
                            div { class: "skeleton", style: "height: 40px; margin-bottom: 16px;" }
                            div { class: "skeleton", style: "height: 80px; margin-bottom: 16px;" }
                        }
                    },
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct DeleteCollectionModalProps {
    update_signal: Signal<()>,
    collection_uuid: CollectionUuid,
}

#[component]
pub fn DeleteCollectionModal(props: DeleteCollectionModalProps) -> Element {
    let collection_uuid = props.collection_uuid;
    let mut update_signal = props.update_signal;
    let mut status_message = use_signal(|| String::new());

    // Fetch collection details to show the collection name
    let collection_future = use_resource(move || async move {
        get_collection(&GetCollectionReq {
            collection_uuid: collection_uuid,
        })
        .await
    });

    let collection_name = match &*collection_future.read() {
        Some(Ok(result)) => result.collection.name.clone(),
        _ => format!("Collection #{}", collection_uuid),
    };

    let footer = rsx! {
        span { class: "status-message", "{status_message}" }
        div {
            class: "modal-buttons",
            style: "display: flex; gap: var(--space-4); justify-content: flex-end;",
            button {
                class: "btn btn-secondary",
                onclick: move |_| {
                    MODAL_STACK.with_mut(|v| v.pop());
                },
                "Cancel"
            }
            button {
                class: "btn btn-danger",
                onclick: move |_| async move {
                    match delete_collection(
                            &DeleteCollectionReq {
                                collection_uuid: collection_uuid,
                            },
                        )
                        .await
                    {
                        Ok(_) => {
                            status_message.set("Collection deleted successfully".into());
                            update_signal.set(());
                            let task = Timeout::new(
                                1500,
                                move || {
                                    MODAL_STACK.with_mut(|v| v.pop());
                                },
                            );
                            task.forget();
                        }
                        Err(err) => {
                            status_message.set(format!("Error: {}", err));
                        }
                    }
                },
                "Delete Collection"
            }
        }
    };

    rsx! {
        ModernModal {
            title: "Confirm Collection Deletion",
            size: ModalSize::Small,
            footer,

            div { class: "confirmation-content",
                p {
                    class: "confirmation-message",
                    style: "margin-bottom: var(--space-4);",
                    "Are you sure you want to delete the collection \"{collection_name}\"? This action cannot be undone."
                }

                div {
                    class: "warning-message",
                    style: "
                        padding: var(--space-3);
                        background-color: rgba(239, 68, 68, 0.1);
                        border-left: 3px solid var(--error);
                        border-radius: var(--radius-md);
                        color: var(--text-secondary);
                    ",
                    "Note: This will only delete the collection. The media files within the collection will remain in your library."
                }
            }
        }
    }
}

// Add to Collection Modal component
#[derive(Clone, PartialEq, Props)]
pub struct AddMediaToCollectionModalProps {
    update_signal: Signal<()>,
    media_uuid: MediaUuid,
}

#[component]
pub fn AddMediaToCollectionModal(props: AddMediaToCollectionModalProps) -> Element {
    let media_uuid = props.media_uuid;
    let mut update_signal = props.update_signal;
    let mut status_message = use_signal(|| String::new());

    // Search state
    let mut collection_search_signal = use_signal(|| String::new());
    let mut selected_collection = use_signal(|| None::<CollectionUuid>);

    // Fetch collections based on search term
    let collections_future = use_resource(move || async move {
        let filter = collection_search_signal();

        search_collections(&SearchCollectionsReq { filter }).await
    });

    // Handle submission
    let handle_submit = move |_| async move {
        if let Some(collection_uuid) = selected_collection() {
            status_message.set("Adding media to collection...".into());

            match add_media_to_collection(&AddMediaToCollectionReq {
                collection_uuid,
                media_uuid,
            })
            .await
            {
                Ok(_) => {
                    status_message.set("Media added to collection successfully".into());
                    update_signal.set(());

                    // Close the modal after a short delay
                    let task = Timeout::new(1500, move || {
                        MODAL_STACK.with_mut(|v| v.pop());
                    });
                    task.forget();
                }
                Err(err) => {
                    status_message.set(format!("Error: {}", err));
                }
            }
        } else {
            status_message.set("Please select an collection first".into());
        }
    };

    let footer = rsx! {
        span { class: "status-message", style: "color: var(--primary);", "{status_message}" }
        div {
            class: "modal-buttons",
            style: "display: flex; gap: var(--space-4); justify-content: flex-end;",
            button {
                class: "btn btn-secondary",
                onclick: move |_| {
                    MODAL_STACK.with_mut(|v| v.pop());
                },
                "Cancel"
            }
            button {
                class: "btn btn-primary",
                disabled: selected_collection().is_none(),
                onclick: handle_submit,
                "Add to Collection"
            }
        }
    };

    // Get collections data for display
    let collections = match &*collections_future.read() {
        Some(Ok(response)) => Some(response.collections.clone()),
        Some(Err(_)) => None,
        None => None,
    };

    rsx! {
        ModernModal { title: "Add to Collection", size: ModalSize::Medium, footer,
            div {
                div {
                    class: "search-bar",
                    style: "
                        display: flex;
                        align-items: center;
                        gap: var(--space-2);
                        margin-bottom: var(--space-6);
                        background-color: var(--surface);
                        padding: var(--space-3);
                        border-radius: var(--radius-lg);
                        box-shadow: var(--shadow-sm);
                    ",
                    form {
                        class: "form-group",
                        style: "width: 100%;",
                        onsubmit: move |event| async move {
                            let filter = match event.values().get("search_filter") {
                                Some(val) => val.as_value(),
                                None => String::from(""),
                            };
                            collection_search_signal.set(filter.clone());
                        },
                        label { class: "form-label", "Search Collections" }
                        div { style: "display: flex; gap: var(--space-2); align-items: center;",
                            input {
                                class: "form-input",
                                r#type: "text",
                                name: "search_filter",
                                value: "{collection_search_signal()}",
                                placeholder: "Enter collection name or description...",
                                style: "flex: 1;",
                            }
                            button { class: "btn btn-primary", r#type: "submit", "Search" }
                        }
                    }
                }

                // Collections list
                div {
                    class: "collections-list",
                    style: "
                        margin-top: var(--space-4);
                        max-height: 300px;
                        overflow-y: auto;
                        border: 1px solid var(--border);
                        border-radius: var(--radius-md);
                    ",

                    match collections {
                        Some(collections) => {
                            if collections.is_empty() {
                                rsx! {
                                    div {
                                        class: "empty-state",
                                        style: "
                                            padding: var(--space-6);
                                            text-align: center;
                                            color: var(--text-tertiary);
                                        ",
                                        "No collections found. Try a different search term or create a new collection."
                                    }
                                }
                            } else {
                                rsx! {
                                    for collection_uuid in collections {
                                        CollectionSelectionItem {
                                            key: "{collection_uuid}",
                                            collection_uuid,
                                            is_selected: selected_collection() == Some(collection_uuid),
                                            on_select: move |_| selected_collection.set(Some(collection_uuid)),
                                        }
                                    }
                                }
                            }
                        }
                        None => {
                            rsx! {
                                // Loading state
                                for _ in 0..3 {
                                    div { class: "skeleton", style: "height: 60px; margin-bottom: 8px;" }
                                }
                            }
                        }
                    }
                }

                // Create new collection button
                div { style: "margin-top: var(--space-4); text-align: center;",
                    button {
                        class: "btn btn-secondary",
                        onclick: move |_| {
                            MODAL_STACK
                                .with_mut(|v| {
                                    v.pop();
                                    v.push(crate::components::modal::Modal::CreateCollection);
                                });
                        },
                        "Create New Collection"
                    }
                }
            }
        }
    }
}

// Helper component for collection selection items
#[derive(Clone, PartialEq, Props)]
struct CollectionSelectionItemProps {
    collection_uuid: CollectionUuid,
    is_selected: bool,
    on_select: EventHandler<MouseEvent>,
}

#[component]
fn CollectionSelectionItem(props: CollectionSelectionItemProps) -> Element {
    let collection_uuid = props.collection_uuid;
    let is_selected = props.is_selected;

    // Fetch collection details
    let collection_future =
        use_resource(
            move || async move { get_collection(&GetCollectionReq { collection_uuid }).await },
        );

    let collection = &*collection_future.read();

    match collection {
        Some(Ok(result)) => {
            let collection = result.collection.clone();
            let description = if collection.note.is_empty() {
                "No description"
            } else {
                &collection.note
            };

            rsx! {
                div {
                    class: if is_selected { "collection-item selected" } else { "collection-item" },
                    style: {
                        let base_style = "
                                padding: var(--space-3);
                                border-bottom: 1px solid var(--border);
                                display: flex;
                                align-items: center;
                                cursor: pointer;
                                transition: background-color var(--transition-fast) var(--easing-standard);
                            ";
                        if is_selected {
                            format!(
                                "{}background-color: var(--primary-light); color: white;",
                                base_style,
                            )
                        } else {
                            base_style.to_string()
                        }
                    },
                    onclick: move |evt| props.on_select.call(evt),

                    // Radio button indicator
                    div { style: "margin-right: var(--space-3);",
                        div {
                            style: {
                                let border_color = if is_selected { "white" } else { "var(--neutral-400)" };
                                format!("
                                    width: 18px;
                                    height: 18px;
                                    border-radius: 50%;
                                    border: 2px solid {};
                                    display: flex;
                                    align-items: center;
                                    justify-content: center;
                                ",
                                    border_color,
                                )
                            },
                            if is_selected {

                                div { style: "
                                        width: 10px;
                                        height: 10px;
                                        border-radius: 50%;
                                        background-color: white;
                                    " }
                            }
                        }
                    }

                    // Collection info
                    div { style: "flex: 1;",
                        div { style: "font-weight: 500;", "{collection.name}" }
                        div {
                            style: {
                                if is_selected {
                                    "font-size: 0.875rem; color: rgba(255, 255, 255, 0.9);"
                                } else {
                                    "font-size: 0.875rem; color: var(--text-tertiary);"
                                }
                            },
                            "Group: {collection.gid} • {description}"
                        }
                    }
                }
            }
        }
        Some(Err(_)) => {
            rsx! {
                div {
                    class: "collection-item error",
                    style: "padding: var(--space-3); border-bottom: 1px solid var(--border); color: var(--error);",
                    "Error loading collection #{collection_uuid}"
                }
            }
        }
        None => {
            rsx! {
                div {
                    class: "collection-item loading",
                    style: "padding: var(--space-3); border-bottom: 1px solid var(--border);",
                    div {
                        class: "skeleton",
                        style: "height: 24px; width: 60%; margin-bottom: 4px;",
                    }
                    div { class: "skeleton", style: "height: 16px; width: 80%;" }
                }
            }
        }
    }
}

// Confirmation modal for removing media from collections
#[derive(Clone, PartialEq, Props)]
pub struct RmFromCollectionModalProps {
    update_signal: Signal<()>,
    media_uuid: MediaUuid,
    collection_uuid: CollectionUuid,
}

#[component]
pub fn RmFromCollectionModal(props: RmFromCollectionModalProps) -> Element {
    let media_uuid = props.media_uuid;
    let collection_uuid = props.collection_uuid;
    let mut update_signal = props.update_signal;
    let mut status_message = use_signal(|| String::new());

    // Fetch collection details to show the collection name
    let collection_future = use_resource(move || async move {
        get_collection(&GetCollectionReq {
            collection_uuid: collection_uuid,
        })
        .await
    });

    let collection_name = match &*collection_future.read() {
        Some(Ok(result)) => result.collection.name.clone(),
        _ => format!("Collection #{}", collection_uuid),
    };

    let footer = rsx! {
        span { class: "status-message", "{status_message}" }
        div {
            class: "modal-buttons",
            style: "display: flex; gap: var(--space-4); justify-content: flex-end;",
            button {
                class: "btn btn-secondary",
                onclick: move |_| {
                    MODAL_STACK.with_mut(|v| v.pop());
                },
                "Cancel"
            }
            button {
                class: "btn btn-danger",
                onclick: move |_| async move {
                    match rm_media_from_collection(
                            &RmMediaFromCollectionReq {
                                collection_uuid: collection_uuid,
                                media_uuid: media_uuid,
                            },
                        )
                        .await
                    {
                        Ok(_) => {
                            status_message.set("Media removed from collection".into());
                            update_signal.set(());
                            let task = Timeout::new(
                                1500,
                                move || {
                                    MODAL_STACK.with_mut(|v| v.pop());
                                },
                            );
                            task.forget();
                        }
                        Err(err) => {
                            status_message.set(format!("Error: {}", err));
                        }
                    }
                },
                "Remove from Collection"
            }
        }
    };

    rsx! {
        ModernModal { title: "Confirm Removal", size: ModalSize::Small, footer,

            div { class: "confirmation-content",
                p { class: "confirmation-message",
                    "Are you sure you want to remove this media from \"{collection_name}\"? The media will still exist in your library."
                }

                div {
                    class: "media-info",
                    style: "margin-top: var(--space-4); padding: var(--space-3); background-color: var(--neutral-50); border-radius: var(--radius-md);",
                    p { "Media ID: {media_uuid}" }
                    p { "Collection: {collection_name} (ID: {collection_uuid})" }
                }
            }
        }
    }
}
