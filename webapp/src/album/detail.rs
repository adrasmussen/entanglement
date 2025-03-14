use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::{
    album::MEDIA_SEARCH_KEY,
    common::{local_time, storage::*},
    components::{
        media_card::MediaCard,
        modal::{Modal, ModalBox, MODAL_STACK},
        search_bar::SearchBar,
    },
    Route,
};
use api::album::*;

#[derive(Clone, PartialEq, Props)]
pub struct AlbumDetailProps {
    // This is a String because we get it from the Router
    album_uuid: String,
}

#[component]
pub fn AlbumDetail(props: AlbumDetailProps) -> Element {
    let update_signal = use_signal(|| ());

    rsx! {
        ModalBox { update_signal }
        ErrorBoundary {
            handle_error: |error: ErrorContext| {
                rsx! {
                    if let Some(error_ui) = error.show() {
                        {error_ui}
                    } else {
                        div { "AlbumDetail encountered an error.  Check the logs or reach out the the administrators." }
                    }
                }
            },
            AlbumInner { update_signal, album_uuid: props.album_uuid }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct AlbumErrorProps {
    message: String,
}

#[component]
fn AlbumError(props: AlbumErrorProps) -> Element {
    rsx! {
        div { class: "container error-state",
            h1 { "Error Loading Album" }
            p { "There was an error loading the media: {props.message}" }
            Link { to: Route::AlbumSearch {}, class: "btn btn-primary", "Return to Albums" }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
struct AlbumInnerProps {
    update_signal: Signal<()>,
    album_uuid: String,
}

#[component]
fn AlbumInner(props: AlbumInnerProps) -> Element {
    let update_signal = props.update_signal;
    let album_uuid = props.album_uuid.parse::<AlbumUuid>().show(|_| {
        let message = "The album_uuid could not be parsed".to_string();
        rsx! {
            AlbumError { message }
        }
    })?;

    // see GalleryInner for details
    let album_uuid = use_memo(use_reactive(&album_uuid, |album_uuid| album_uuid));
    let album_future = use_resource(move || async move {
        let album_uuid = album_uuid();
        get_album(&GetAlbumReq { album_uuid }).await
    });

    let media_search_signal = use_signal::<String>(|| try_local_storage(MEDIA_SEARCH_KEY));
    let media_future = use_resource(move || async move {
        update_signal();
        let album_uuid = album_uuid();
        let filter = media_search_signal();

        search_media_in_album(&SearchMediaInAlbumReq { album_uuid, filter }).await
    });

    // see GalleryInner for details
    //
    // the two futures both early return the same loading skeleton, but they could differ in principle
    let album_data = &*album_future.read();
    let album_data = match album_data.clone().transpose().show(|error| {
        rsx! {
            AlbumError { message: format!("There was an error loading the album: {error}") }
        }
    })? {
        Some(v) => v,
        None => {
            return rsx! {
                AlbumDetailSkeleton {}
            }
        }
    };

    let media_data = &*media_future.read();
    let media_data = match media_data.clone().transpose().show(|error| {
        rsx! {
            AlbumError { message: format!("There was an error searching media in the album: {error}") }
        }
    })? {
        Some(v) => v,
        None => {
            return rsx! {
                AlbumDetailSkeleton {}
            }
        }
    };

    let album = album_data.album;
    let media = media_data.media;

    let formatted_time = local_time(album.mtime);

    rsx! {
        div { class: "container",
            // breadcrumb navigation
            div { class: "breadcrumb", style: "margin-bottom: var(--space-4);",
                Link { to: Route::AlbumSearch {}, "Albums" }
                span { " / " }
                span { "{album.name}" }
            }
            // album detail view header
            div {
                class: "album-detail-header",
                style: "
                    background-color: var(--surface);
                    border-radius: var(--radius-lg);
                    padding: var(--space-4);
                    margin-bottom: var(--space-4);
                    box-shadow: var(--shadow-sm);
                ",
                div { style: "display: flex; justify-content: space-between; align-items: flex-start;",
                    // Album info
                    div {
                        h1 { style: "margin: 0 0 var(--space-2) 0;", "{album.name}" }
                        div { style: "
                                display: flex;
                                gap: var(--space-4);
                                margin-bottom: var(--space-3);
                                color: var(--text-secondary);
                                font-size: 0.875rem;
                            ",
                            span { "Owner: {album.uid}" }
                            span { "Group: {album.gid}" }
                            span { "Last modified: {formatted_time}" }
                        }

                        if !album.note.is_empty() {
                            p { style: "
                                    padding: var(--space-3);
                                    background-color: var(--neutral-50);
                                    border-radius: var(--radius-md);
                                    font-style: italic;
                                    color: var(--text-secondary);
                                    max-width: 700px;
                                ",
                                "{album.note}"
                            }
                        }
                    }
                    // Action buttons
                    div { style: "display: flex; gap: var(--space-2);",
                        button {
                            class: "btn btn-secondary",
                            onclick: move |_| {
                                MODAL_STACK.with_mut(|v| v.push(Modal::EditAlbum(album_uuid())));
                            },
                            "Edit Album"
                        }
                        button {
                            class: "btn btn-danger",
                            onclick: move |_| {
                                MODAL_STACK.with_mut(|v| v.push(Modal::DeleteAlbum(album_uuid())));
                            },
                            "Delete Album"
                        }
                    }
                }
            }
            SearchBar {
                search_signal: media_search_signal,
                storage_key: MEDIA_SEARCH_KEY,
                placeholder: "Search media in this album...",
                status: format!("Found {} items in this album", media.len()),
            }
            // media grid
            if media.is_empty() {
                div {
                    class: "empty-state",
                    style: "
                        padding: var(--space-8) var(--space-4);
                        text-align: center;
                        background-color: var(--surface);
                        border-radius: var(--radius-lg);
                        margin-top: var(--space-4);
                    ",
                    div { style: "
                            font-size: 4rem;
                            margin-bottom: var(--space-4);
                            color: var(--neutral-400);
                        ",
                        "🖼️"
                    }
                    h3 { style: "
                            margin-bottom: var(--space-2);
                            color: var(--text-primary);
                        ",
                        "No Media in This Album"
                    }
                    p { style: "
                            color: var(--text-secondary);
                            max-width: 500px;
                            margin: 0 auto;
                        ",
                        "This album doesn't contain any media yet. Add some media to get started."
                    }
                    button {
                        class: "btn btn-primary",
                        style: "margin-top: var(--space-4);",
                        onclick: move |_| {},
                        "Add Media to Album"
                    }
                }
            } else {
                div {
                    class: "media-grid",
                    style: "
                        display: grid;
                        grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
                        gap: var(--space-4);
                        margin-top: var(--space-4);
                    ",
                    for media_uuid in media.iter() {
                        MediaCard {
                            key: "{media_uuid}",
                            media_uuid: *media_uuid,
                            album_uuid: Some(album_uuid()),
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn AlbumDetailSkeleton() -> Element {
    rsx! {
        div { class: "container loading-state",
            div {
                class: "skeleton",
                style: "height: 40px; width: 200px; margin-bottom: 16px;",
            }
            div {
                class: "skeleton",
                style: "height: 200px; margin-bottom: 16px;",
            }
            div {
                class: "skeleton",
                style: "height: 60px; margin-bottom: 16px;",
            }
            div { style: "
                    display: grid;
                    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
                    gap: var(--space-4);
                ",
                for _ in 0..6 {
                    div { class: "skeleton", style: "height: 200px;" }
                }
            }
        }
    }
}
