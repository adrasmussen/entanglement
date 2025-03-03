use dioxus::prelude::*;
use gloo_timers::callback::Timeout;

use crate::components::modal::{MODAL_STACK, ModalSize, ModernModal};
use api::{
    album::{
        AddAlbumReq, AlbumUuid, GetAlbumReq, RmMediaFromAlbumReq, add_album, get_album,
        rm_media_from_album,
    },
    media::MediaUuid,
};

// Confirmation modal for removing media from albums
#[derive(Clone, PartialEq, Props)]
pub struct RmFromAlbumModalProps {
    update_signal: Signal<()>,
    media_uuid: MediaUuid,
    album_uuid: AlbumUuid,
}

#[component]
pub fn RmFromAlbumModal(props: RmFromAlbumModalProps) -> Element {
    let media_uuid = props.media_uuid;
    let album_uuid = props.album_uuid;
    let mut update_signal = props.update_signal;
    let mut status_message = use_signal(|| String::new());

    // Fetch album details to show the album name
    let album_future = use_resource(move || async move {
        get_album(&GetAlbumReq {
            album_uuid: album_uuid,
        })
        .await
    });

    let album_name = match &*album_future.read() {
        Some(Ok(result)) => result.album.name.clone(),
        _ => format!("Album #{}", album_uuid),
    };

    let footer = rsx! {
        span { class: "status-message", "{status_message}" }
        div { class: "modal-buttons",
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
                    match rm_media_from_album(
                            &RmMediaFromAlbumReq {
                                album_uuid: album_uuid,
                                media_uuid: media_uuid,
                            },
                        )
                        .await
                    {
                        Ok(_) => {
                            status_message.set("Media removed from album".into());
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
                "Remove from Album"
            }
        }
    };

    rsx! {
        ModernModal { title: "Confirm Removal", size: ModalSize::Small, footer,

            div { class: "confirmation-content",
                p { class: "confirmation-message",
                    "Are you sure you want to remove this media from \"{album_name}\"? The media will still exist in your library."
                }

                div {
                    class: "media-info",
                    style: "margin-top: var(--space-4); padding: var(--space-3); background-color: var(--neutral-50); border-radius: var(--radius-md);",
                    p { "Media ID: {media_uuid}" }
                    p { "Album: {album_name} (ID: {album_uuid})" }
                }
            }
        }
    }
}

// Create Album Modal
#[derive(Clone, PartialEq, Props)]
pub struct CreateAlbumModalProps {
    update_signal: Signal<()>,
}

#[component]
pub fn CreateAlbumModal(props: CreateAlbumModalProps) -> Element {
    let mut update_signal = props.update_signal;
    let mut status_message = use_signal(|| String::new());

    // Form state
    let mut album_name = use_signal(|| String::new());
    let mut album_group = use_signal(|| String::new());
    let mut album_note = use_signal(|| String::new());

    // Form validation state
    let mut name_error = use_signal(|| String::new());
    let mut group_error = use_signal(|| String::new());

    // Handle submission
    let handle_submit = move |_| async move {
        // Reset validation errors
        name_error.set(String::new());
        group_error.set(String::new());

        // Basic validation
        let mut is_valid = true;

        if album_name().trim().is_empty() {
            name_error.set("Album name is required".into());
            is_valid = false;
        }

        if album_group().trim().is_empty() {
            group_error.set("Group ID is required".into());
            is_valid = false;
        }

        if !is_valid {
            return;
        }

        // We're ready to submit
        status_message.set("Creating album...".into());

        match add_album(&AddAlbumReq {
            gid: album_group(),
            name: album_name(),
            note: album_note(),
        })
        .await
        {
            Ok(resp) => {
                status_message.set(format!("Album created with ID: {}", resp.album_uuid));
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
        div { class: "modal-buttons",
            button {
                class: "btn btn-secondary",
                onclick: move |_| {
                    MODAL_STACK.with_mut(|v| v.pop());
                },
                "Cancel"
            }
            button { class: "btn btn-primary", onclick: handle_submit, "Create Album" }
        }
    };

    rsx! {
        ModernModal { title: "Create New Album", size: ModalSize::Medium, footer,
            div { class: "create-album-form",
                div { class: "form-group",
                    label { class: "form-label", "Album Name" }
                    input {
                        class: "form-input",
                        r#type: "text",
                        value: "{album_name}",
                        oninput: move |evt| album_name.set(evt.value().clone()),
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
                    input {
                        class: "form-input",
                        r#type: "text",
                        value: "{album_group}",
                        oninput: move |evt| album_group.set(evt.value().clone()),
                        placeholder: "users",
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
                        "Group ID determines who can access this album"
                    }
                }

                div { class: "form-group",
                    label { class: "form-label", "Description (optional)" }
                    textarea {
                        class: "form-textarea",
                        rows: 3,
                        value: "{album_note}",
                        oninput: move |evt| album_note.set(evt.value().clone()),
                        placeholder: "Add a description for this album...",
                    }
                }
            }
        }
    }
}
