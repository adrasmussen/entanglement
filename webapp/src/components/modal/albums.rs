use dioxus::prelude::*;

use crate::components::modal::{ModernModal, MODAL_STACK, ModalSize};
use api::{
    album::{get_album, rm_media_from_album, AlbumUuid, GetAlbumReq, RmMediaFromAlbumReq},
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
                            MODAL_STACK.with_mut(|v| v.pop());
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
