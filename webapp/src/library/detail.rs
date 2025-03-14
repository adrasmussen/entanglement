// webapp/src/library/detail.rs

use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::{
    common::{local_time, storage::*},
    components::{modal::ModalBox, search_bar::SearchBar},
    library::MEDIA_SEARCH_KEY,
    Route,
};
use api::library::*;

#[derive(Clone, PartialEq, Props)]
pub struct LibraryDetailProps {
    // This is a String because we get it from the Router
    library_uuid: String,
}

#[component]
pub fn LibraryDetail(props: LibraryDetailProps) -> Element {
    let update_signal = use_signal(|| ());

    rsx! {
        ModalBox { update_signal }
        ErrorBoundary {
            handle_error: |error: ErrorContext| {
                rsx! {
                    if let Some(error_ui) = error.show() {
                        {error_ui}
                    } else {
                        div { "LibraryDetail encountered an error.  Check the logs or reach out the the administrators." }
                    }
                }
            },
            LibraryInner { library_uuid: props.library_uuid }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct LibraryErrorProps {
    message: String,
}

#[component]
fn LibraryError(props: LibraryErrorProps) -> Element {
    rsx! {
        div { class: "container error-state",
            h1 { "Error Loading Library" }
            p { "There was an error loading the library: {props.message}" }
            Link { to: Route::LibrarySearch {}, class: "btn btn-primary",
                "Return to Libraries"
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
struct LibraryInnerProps {
    library_uuid: String,
}

#[component]
fn LibraryInner(props: LibraryInnerProps) -> Element {
    let update_signal = use_signal(|| ());
    let library_uuid = props.library_uuid.parse::<LibraryUuid>().show(|_| {
        let message = "The library_uuid could not be parsed".to_string();
        rsx! {
            LibraryError { message }
        }
    })?;

    // see GalleryInner for details
    let library_uuid = use_memo(use_reactive(&library_uuid, |library_uuid| library_uuid));
    let library_future = use_resource(move || async move {
        let library_uuid = library_uuid();
        get_library(&GetLibraryReq { library_uuid }).await
    });

    // the library media search is the only place where we can specify hidden = true
    let mut show_hidden = use_signal(|| false);
    let media_search_signal = use_signal::<String>(|| try_local_storage(MEDIA_SEARCH_KEY));
    let media_future = use_resource(move || async move {
        update_signal();
        let library_uuid = library_uuid();
        let filter = media_search_signal();
        let hidden = show_hidden();

        search_media_in_library(&SearchMediaInLibraryReq {
            library_uuid,
            filter,
            hidden,
        })
        .await
    });

    // see GalleryInner for details
    //
    // the two futures both early return the same loading skeleton, but they could differ in principle
    let library_data = &*library_future.read();
    let library_data = match library_data.clone().transpose().show(|error| {
        rsx! {
            LibraryError { message: format!("There was an error loading the library: {error}") }
        }
    })? {
        Some(v) => v,
        None => {
            return rsx! {
                LibraryDetailSkeleton {}
            }
        }
    };

    let media_data = &*media_future.read();
    let media_data = match media_data.clone().transpose().show(|error| {
        rsx! {
            LibraryError { message: format!("There was an error searching media in the library: {error}") }
        }
    })? {
        Some(v) => v,
        None => {
            return rsx! {
                LibraryDetailSkeleton {}
            }
        }
    };

    // search bar action button
    let action_button = rsx! {
        div { style: "display: flex; align-items: center; margin-left: auto;",
            // Checkbox for hidden files
            input {
                r#type: "checkbox",
                id: "show-hidden-checkbox",
                checked: show_hidden(),
                oninput: move |evt| {
                    show_hidden.set(evt.checked());
                },
                style: "margin: 0 8px 0 0;",
            }
            label { r#for: "show-hidden-checkbox", style: "margin: 0 16px 0 0;", "Show hidden files" }
        }
    };

    let library = library_data.library;
    let media = media_data.media;

    let formatted_time = local_time(library.mtime);

    rsx! {
        div { class: "container",
            // breadcrumb navigation
            div {
                class: "breadcrumb",
                style: "margin-bottom: var(--space-4);",
                Link { to: Route::LibrarySearch {}, "Libraries" }
                span { " / " }
                span { "{library.path}" }
            }
            // library detail view header
            div {
                class: "library-detail-header",
                style: "
                    background-color: var(--surface);
                    border-radius: var(--radius-lg);
                    padding: var(--space-4);
                    margin-bottom: var(--space-4);
                    box-shadow: var(--shadow-sm);
                ",
                div { style: "display: flex; justify-content: space-between; align-items: flex-start;",
                    // Library info
                    div {
                        h1 { style: "margin: 0 0 var(--space-2) 0;", "Library: {library.path}" }
                        div { style: "
                                display: flex;
                                gap: var(--space-4);
                                margin-bottom: var(--space-3);
                                color: var(--text-secondary);
                                font-size: 0.875rem;
                            ",
                            span { "Owner: {library.uid}" }
                            span { "Group: {library.gid}" }
                            span { "Last scanned: {formatted_time}" }
                            span { "File count: {library.count}" }
                        }
                    }
                    // Action buttons
                    div { style: "display: flex; gap: var(--space-2);",
                        button {
                            class: "btn btn-secondary",
                            onclick: move |_| {
                                // Placeholder for scan action
                                tracing::info!("Start scan for library {library_uuid}");
                            },
                            "Scan Library"
                        }
                    }
                }
            }
            SearchBar {
                search_signal: media_search_signal,
                storage_key: MEDIA_SEARCH_KEY,
                placeholder: "Search media in this library...",
                status: format!(
                    "Found {} items in this library{}",
                    media.len(),
                    if show_hidden() { " (including hidden)" } else { "" },
                ),
                action_button,
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
                        "No Media Found"
                    }
                    p { style: "
                            color: var(--text-secondary);
                            max-width: 500px;
                            margin: 0 auto;
                        ",
                        if show_hidden() {
                            "No media matches your search criteria in this library."
                        } else {
                            "No media matches your search criteria. Try different search terms or check the 'Show hidden files' option."
                        }
                    }
                    if !show_hidden() {
                        button {
                            class: "btn btn-secondary",
                            style: "margin-top: var(--space-4);",
                            onclick: move |_| {
                                show_hidden.set(true);
                            },
                            "Show Hidden Files"
                        }
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
                        crate::components::media_card::MediaCard {
                            key: "{media_uuid}",
                            media_uuid: *media_uuid,
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn LibraryDetailSkeleton() -> Element {
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
