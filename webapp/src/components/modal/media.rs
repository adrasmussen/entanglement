use std::collections::HashSet;

use dioxus::prelude::*;
use tracing::error;

use crate::components::modal::{ModalSize, ModernModal, ProgressBar, MODAL_STACK};
use api::{full_link, media::*, unfold_set, FOLDING_SEPARATOR};

#[derive(Clone, PartialEq, Props)]
pub struct EnhancedMediaModalProps {
    media_uuid: MediaUuid,
}

#[component]
pub fn EnhancedMediaModal(props: EnhancedMediaModalProps) -> Element {
    let media_uuid = props.media_uuid;

    // State for image pan and zoom
    let mut zoom_level = use_signal(|| 1.0);
    let mut is_panning = use_signal(|| false);
    let mut translate_x = use_signal(|| 0.0);
    let mut translate_y = use_signal(|| 0.0);
    let mut start_pos_x = use_signal(|| 0.0);
    let mut start_pos_y = use_signal(|| 0.0);

    // Fetch media data
    let media_future =
        use_resource(move || async move { get_media(&GetMediaReq { media_uuid }).await });

    let get_transform_style = move || {
        format!(
            "transform: scale({}) translate({}px, {}px);",
            zoom_level(),
            translate_x(),
            translate_y()
        )
    };

    // Helper functions for zoom controls
    let mut zoom_in = move |_| {
        if zoom_level() < 3.0 {
            zoom_level.set(zoom_level() + 0.25);
        }
    };

    let mut zoom_out = move |_| {
        if zoom_level() > 0.5 {
            zoom_level.set(zoom_level() - 0.25);

            // Reset translation if we're back to normal size
            if zoom_level() <= 1.0 {
                translate_x.set(0.0);
                translate_y.set(0.0);
            }
        }
    };

    let mut reset_zoom = move |_| {
        zoom_level.set(1.0);
        translate_x.set(0.0);
        translate_y.set(0.0);
    };

    let element = match &*media_future.read() {
        Some(Ok(media_data)) => {
            let media = media_data.media.clone();

            match media.metadata {
                MediaMetadata::Image => {
                    rsx! {
                        ModernModal {
                            title: "Image Viewer",
                            size: crate::components::modal::ModalSize::Full,

                            footer: rsx! {
                                button {
                                    class: "btn btn-primary",
                                    onclick: move |_| {
                                        MODAL_STACK.with_mut(|v| v.pop());
                                    },
                                    "Close"
                                }
                            },

                            div { class: "fullsize-image-container",
                                img {
                                    src: full_link(media_uuid),
                                    alt: media.note.clone(),
                                    class: if is_panning() { "fullsize-image panning".to_string() } else if zoom_level() > 1.0 { "fullsize-image zoomed".to_string() } else { "fullsize-image".to_string() },
                                    style: get_transform_style(),

                                    // Mouse event handlers for panning
                                    onmousedown: move |event| {
                                        if zoom_level() > 1.0 {
                                            is_panning.set(true);
                                            start_pos_x.set(event.client_coordinates().x as f64);
                                            start_pos_y.set(event.client_coordinates().y as f64);
                                        }
                                    },
                                    onmousemove: move |event| {
                                        if is_panning() {
                                            let current_x = event.client_coordinates().x as f64;
                                            let current_y = event.client_coordinates().y as f64;
                                            let delta_x = current_x - start_pos_x();
                                            let delta_y = current_y - start_pos_y();
                                            translate_x.set(translate_x() + delta_x / zoom_level());
                                            translate_y.set(translate_y() + delta_y / zoom_level());
                                            start_pos_x.set(current_x);
                                            start_pos_y.set(current_y);
                                        }
                                    },
                                    onmouseup: move |_| {
                                        is_panning.set(false);
                                    },
                                    onmouseleave: move |_| {
                                        is_panning.set(false);
                                    },

                                    // Double-click to reset zoom
                                    ondoubleclick: move |_| {
                                        reset_zoom(());
                                    },
                                }

                                // Zoom controls
                                div { class: "zoom-controls",
                                    button {
                                        class: "zoom-button",
                                        onclick: move |_| zoom_out(()),
                                        "-"
                                    }
                                    span { class: "zoom-level", "{(zoom_level() * 100.0) as i32}%" }
                                    button {
                                        class: "zoom-button",
                                        onclick: move |_| zoom_in(()),
                                        "+"
                                    }
                                    button {
                                        class: "zoom-button",
                                        onclick: move |_| reset_zoom(()),
                                        "↺"
                                    }
                                }
                            }
                        }
                    }
                }
                MediaMetadata::Video => {
                    // Handle video differently - full screen with controls
                    rsx! {
                        ModernModal {
                            title: "Video Player",
                            size: crate::components::modal::ModalSize::Large,

                            footer: rsx! {
                                button {
                                    class: "btn btn-primary",
                                    onclick: move |_| {
                                        MODAL_STACK.with_mut(|v| v.pop());
                                    },
                                    "Close"
                                }
                            },

                            div { class: "video-player-container",
                                video {
                                    src: full_link(media_uuid),
                                    controls: true,
                                    autoplay: true,
                                    class: "fullsize-video",
                                }
                            }
                        }
                    }
                }
                _ => {
                    // Unsupported media type
                    rsx! {
                        ModernModal {
                            title: "Unsupported Media",
                            size: crate::components::modal::ModalSize::Medium,

                            footer: rsx! {
                                button {
                                    class: "btn btn-primary",
                                    onclick: move |_| {
                                        MODAL_STACK.with_mut(|v| v.pop());
                                    },
                                    "Close"
                                }
                            },

                            div { class: "error-state",
                                p { "This media type is not supported for preview." }
                            }
                        }
                    }
                }
            }
        }
        Some(Err(err)) => {
            rsx! {
                ModernModal {
                    title: "Error",
                    size: crate::components::modal::ModalSize::Small,

                    footer: rsx! {
                        button {
                            class: "btn btn-primary",
                            onclick: move |_| {
                                MODAL_STACK.with_mut(|v| v.pop());
                            },
                            "Close"
                        }
                    },

                    div { class: "error-state",
                        p { "Failed to load media: {err}" }
                    }
                }
            }
        }
        None => {
            rsx! {
                ModernModal {
                    title: "Loading...",
                    size: crate::components::modal::ModalSize::Medium,

                    div { class: "loading-state skeleton-loader",
                        div { class: "skeleton", style: "height: 300px;" }
                    }
                }
            }
        }
    };

    element
}

#[derive(Clone, PartialEq, Props)]
pub struct BulkAddTagsModalProps {
    update_signal: Signal<()>,
    media_uuids: HashSet<MediaUuid>,
}

#[component]
pub fn BulkEditTagsModal(props: BulkAddTagsModalProps) -> Element {
    let mut update_signal = props.update_signal;

    let mut edit_tags = use_signal(|| String::new());

    let edit_mode_signal = use_signal(|| TagEditMode::Add);

    let mut status_signal = use_signal(|| String::new());

    let media_uuids = props.media_uuids.clone();

    let mut processing_count = use_signal(|| 0);
    let mut success_count = use_signal(|| 0);
    let mut error_count = use_signal(|| 0);
    let media_count = media_uuids.len() as i64;

    let handle_submit = move |_| {
        let media_uuids = media_uuids.clone();
        async move {
            status_signal.set(format!("Adding tags on {} media items...", media_count));

            processing_count.set(0);
            success_count.set(0);
            error_count.set(0);

            let edit_tags = unfold_set(&edit_tags());

            // Process each media item
            for media_uuid in media_uuids.clone() {
                processing_count.set(processing_count() + 1);

                let media = match get_media(&GetMediaReq { media_uuid }).await {
                    Ok(v) => v.media,
                    Err(err) => {
                        error!("failed to get media while bulk editing tags: {err}");
                        error_count.set(error_count() + 1);
                        continue;
                    }
                };

                let tags = media.tags.clone();

                let new_tags: HashSet<String> = match edit_mode_signal() {
                    TagEditMode::Add => {
                        if edit_tags.difference(&tags).collect::<Vec<_>>().len() == 0 {
                            success_count.set(success_count() + 1);
                            continue;
                        }

                        tags.union(&edit_tags).map(|v| v.clone()).collect()
                    }
                    TagEditMode::Remove => {
                        if edit_tags.intersection(&tags).collect::<Vec<_>>().len() == 0 {
                            success_count.set(success_count() + 1);
                            continue;
                        }

                        tags.difference(&edit_tags).map(|v| v.clone()).collect()
                    }
                };

                match update_media(&UpdateMediaReq {
                    media_uuid,
                    update: MediaUpdate {
                        hidden: None,
                        date: None,
                        note: None,
                        tags: Some(new_tags),
                    },
                })
                .await
                {
                    Ok(_) => {
                        success_count.set(success_count() + 1);
                    }
                    Err(err) => {
                        error!("failed to update media while bulk editing tags: {err}");
                        error_count.set(error_count() + 1);
                    }
                }
            }

            // Update overall status
            if error_count() == 0 {
                status_signal.set(format!(
                    "Successfully added tags to {} items",
                    success_count()
                ));
            } else {
                status_signal.set(format!(
                    "Modified {} items, {} failed; see browser console",
                    success_count(),
                    error_count()
                ));
            }

            update_signal.set(());

            // Close the modal after a delay if successful
            if error_count() == 0 {
                let task = gloo_timers::callback::Timeout::new(1500, move || {
                    MODAL_STACK.with_mut(|v| v.pop());
                });
                task.forget();
            }
        }
    };

    let footer = rsx! {
        span { class: "status-message", style: "color: var(--primary);", "{status_signal}" }
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
                disabled: edit_tags().is_empty(),
                onclick: handle_submit,
                "Edit Tags"
            }
        }
    };

    rsx! {
        ModernModal {
            title: format!("Add Tags to {} Items", media_count),
            size: ModalSize::Medium,
            footer,
            div {
                ProgressBar {
                    processing_count,
                    success_count,
                    error_count,
                    media_count,
                }

                p { "Add tags to media" }
                div { class: "task-options",
                    TagEditOption {
                        edit_mode: TagEditMode::Add,
                        edit_mode_signal,
                        title: "Add tags to media",
                        description: "Add tags to media if they are not already present",
                        icon: "🏷️",
                    }
                    TagEditOption {
                        edit_mode: TagEditMode::Remove,
                        edit_mode_signal,
                        title: "Remove tags from media",
                        description: "Remove tags from media if they are present",
                        icon: "🚫",
                    }

                }
                div { class: "form-group",
                    label { class: "form-label" }
                    div { style: "display: flex; gap: var(--space-2);",
                        input {
                            class: "form-input",
                            r#type: "text",
                            value: "{edit_tags()}",
                            oninput: move |evt| edit_tags.set(evt.value().clone()),
                            placeholder: "tag 1|tag 2|...",
                            style: "flex: 1;",
                        }
                    }
                    div {
                        class: "form-help",
                        style: "color: var(--text-tertiary); font-size: 0.875rem; margin-top: 0.25rem;",
                        "Enter tags separated by {FOLDING_SEPARATOR}"
                    }
                }

                // Media count summary
                div { style: "margin-top: var(--space-4); padding: var(--space-3); background-color: var(--neutral-50); border-radius: var(--radius-md);",
                    p { style: "margin: 0; color: var(--text-secondary); font-weight: 500;",
                        "{media_count} items selected for bulk operation"
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum TagEditMode {
    Add,
    Remove,
}

#[derive(Clone, PartialEq, Props)]
struct TagEditOptionProps {
    edit_mode: TagEditMode,
    edit_mode_signal: Signal<TagEditMode>,
    title: String,
    description: String,
    icon: String,
}

#[component]
fn TagEditOption(props: TagEditOptionProps) -> Element {
    let edit_mode = props.edit_mode;
    let mut edit_mode_signal = props.edit_mode_signal;

    rsx! {
        div {
            class: if edit_mode == edit_mode_signal() { "task-option selected" } else { "task-option" },
            onclick: move |_| edit_mode_signal.set(edit_mode),
            div { class: "task-radio",
                div { class: "task-radio-outer",
                    if edit_mode == edit_mode_signal() {
                        div { class: "task-radio-inner" }
                    }
                }
            }
            div { class: "task-icon", "{props.icon}" }
            div { class: "task-info",
                div { class: "task-name", "{props.title}" }
                div { class: "task-description", "{props.description}" }
            }
        }
    }
}
