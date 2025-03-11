use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::{
    common::stream::{full_link, thumbnail_link},
    components::modal::{Modal, ModalBox, MODAL_STACK},
    gallery::{album_details::AlbumDetailsTable, comments::CommentsList},
    Route,
};
use api::{album::AlbumUuid, comment::CommentUuid, media::*};

#[derive(Clone, PartialEq, Props)]
pub struct GalleryDetailProps {
    // This is a String because we get it from the Router
    media_uuid: String,
}

#[component]
pub fn GalleryDetail(props: GalleryDetailProps) -> Element {
    let media_uuid = match props.media_uuid.parse::<MediaUuid>() {
        Ok(uuid) => uuid,
        Err(_) => {
            return rsx! {
                div { class: "error-state container",
                    h1 { "Invalid Media ID" }
                    p { "The provided media ID could not be parsed." }
                    Link { to: Route::GallerySearch {}, class: "btn btn-primary", "Return to Gallery" }
                }
            };
        }
    };

    let mut update_signal = use_signal(|| ());

    // Fetch media data
    let media_future = use_resource(move || async move {
        update_signal.read();

        get_media(&GetMediaReq { media_uuid }).await
    });

    let media_data = &*media_future.read();

    // send the data to the child nodes via signals to ensure correct behavior
    let mut album_uuids = use_signal(|| Vec::<AlbumUuid>::new());
    let mut comment_uuids = use_signal(|| Vec::<CommentUuid>::new());

    // Fetch similar media
    let mut media_hash = use_signal(|| String::new());

    // TODO -- make this a local variable?
    let mut distance = use_signal(|| 128);

    let similar_future = use_resource(move || async move {
        let hash = media_hash();
        let distance = distance();

        similar_media(&SimilarMediaReq { hash, distance }).await
    });

    let similar_media = &*similar_future.read();

    match media_data {
        Some(Ok(media_data)) => {
            let media = media_data.media.clone();

            album_uuids.set(media_data.albums.clone());
            comment_uuids.set(media_data.comments.clone());

            if media_hash() == "".to_string() {
                media_hash.set(media_data.media.hash.clone())
            };

            // Format metadata for display
            let date_formatted = if media.date.is_empty() {
                "Unknown date".to_string()
            } else {
                media.date.clone()
            };

            let mut status_signal = use_signal(|| String::from(""));

            rsx! {
                div { class: "container",
                    ModalBox { update_signal }

                    // Breadcrumb navigation
                    div { class: "breadcrumb",
                        Link { to: Route::GallerySearch {}, "Gallery" }
                        span { " / " }
                        span { "Media Details" }
                    }

                    // New side-by-side layout with independent scrolling
                    div {
                        class: "media-detail-page",
                        style: "
                            display: flex;
                            gap: var(--space-6);
                            position: relative;
                            height: calc(100vh - 160px);
                        ",

                        // Left column - Media display (fixed position)
                        div {
                            class: "media-detail-main",
                            style: "
                                flex: 0 0 50%;
                                max-width: 50%;
                                position: sticky;
                                top: var(--header-height);
                                height: fit-content;
                                max-height: calc(100vh - 140px);
                                overflow: hidden;
                            ",

                            // Image display
                            div { class: "media-detail-view",
                                match media.metadata {
                                    MediaMetadata::Image => rsx! {
                                        img {
                                            src: full_link(media_uuid),
                                            alt: media.note.clone(),
                                            class: "media-detail-image",
                                            style: "
                                                width: 100%;
                                                border-radius: var(--radius-lg);
                                                cursor: pointer;
                                                max-height: calc(100vh - 280px);
                                                object-fit: contain;
                                            ",
                                            onclick: move |_| {
                                                MODAL_STACK.with_mut(|v| v.push(Modal::EnhancedImageView(media_uuid)));
                                            },
                                        }
                                    },
                                    MediaMetadata::Video => rsx! {
                                        video {
                                            controls: true,
                                            src: full_link(media_uuid),
                                            class: "media-detail-video",
                                            style: "
                                                width: 100%;
                                                border-radius: var(--radius-lg);
                                                max-height: calc(100vh - 280px);
                                            ",
                                        }
                                    },
                                    _ => rsx! {
                                        div { class: "unsupported-media", "This media type is not supported for preview" }
                                    },
                                }
                            }

                            div {
                                class: "similar-media-section",
                                style: "
                                    margin-top: var(--space-4);
                                    padding: var(--space-3);
                                    background-color: var(--surface);
                                    border-radius: var(--radius-lg);
                                    box-shadow: var(--shadow-sm);
                                ",

                                h3 { style: "
                                        font-size: 1.125rem;
                                        margin-bottom: var(--space-3);
                                        display: flex;
                                        justify-content: space-between;
                                        align-items: center;
                                    ",
                                    span { "Similar Media" }

                                    // Distance selector for similarity threshold
                                    div { style: "display: flex; align-items: center; gap: var(--space-2);",
                                        span { style: "font-size: 0.875rem; font-weight: normal; color: var(--text-tertiary);",
                                            "Threshold:"
                                        }
                                        select {
                                            style: "
                                                font-size: 0.875rem;
                                                padding: 2px 6px;
                                                border-radius: var(--radius-md);
                                                border: 1px solid var(--border);
                                                background-color: var(--surface);
                                            ",
                                            value: "{distance()}",
                                            onchange: move |evt| {
                                                if let Ok(val) = evt.value().parse::<i64>() {
                                                    distance.set(val);
                                                }
                                            },
                                            option { value: "64", "Very Similar" }
                                            option { value: "128", "Similar" }
                                            option { value: "192", "Somewhat Similar" }
                                            option { value: "256", "Broadly Similar" }
                                        }
                                    }
                                }

                                // Vertical scrollable container for similar media
                                div {
                                    class: "similar-media-container",
                                    style: "
                                        overflow-y: auto;
                                        max-height: 300px; /* Limit height to enforce scrolling */
                                        padding-right: var(--space-2);
                                        /* Enable smooth scrolling */
                                        scroll-behavior: smooth;
                                        /* Hide scrollbar but keep functionality */
                                        scrollbar-width: thin;
                                        scrollbar-color: var(--neutral-300) transparent;

                                        /* Custom scrollbar styling */
                                        &::-webkit-scrollbar {{
                                            width: 6px;
                                       }}
                                        &::-webkit-scrollbar-track {{
                                            background: transparent;
                                        }}
                                        &::-webkit-scrollbar-thumb {{
                                            background-color: var(--neutral-300);
                                            border-radius: 20px;
                                        }}
                                    ",

                                    match similar_media {
                                        Some(Ok(resp)) if !resp.media.is_empty() => {
                                            let similar_items = resp.media.clone();
                                            let filtered_items = similar_items
                                                .iter()
                                                .filter(|uuid| **uuid != media_uuid)
                                                .collect::<Vec<_>>();
                                            rsx! {
                                                if filtered_items.is_empty() {

                                                    div { style: "
                                                            padding: var(--space-4);
                                                            text-align: center;
                                                            color: var(--text-tertiary);
                                                            font-style: italic;
                                                        ",
                                                        "No similar media found. Try adjusting the threshold."
                                                    }
                                                } else {

                                                    div {
                                                        class: "similar-media-grid",
                                                        style: "
                                                            display: grid;
                                                            grid-template-columns: repeat(3, 1fr);
                                                            gap: var(--space-2);
                                                            width: 100%;
                                                        ",

                                                        for & media_id in filtered_items {
                                                            Link {
                                                                key: "{media_id}",
                                                                to: Route::GalleryDetail {
                                                                    media_uuid: media_id.to_string(),
                                                                },
                                                                div {
                                                                    class: "similar-media-item",
                                                                    style: "
                                                                        position: relative;
                                                                        overflow: hidden;
                                                                        border-radius: var(--radius-md);
                                                                        height: 100%;
                                                                        transition: transform var(--transition-fast) var(--easing-standard);

                                                                        &:hover {{
                                                                            transform: scale(1.05);
                                                                            box-shadow: var(--shadow-md);
                                                                            z-index: 1;
                                                                        }}
                                                                    ",
                                                                    img {
                                                                        src: thumbnail_link(media_id),
                                                                        alt: "Similar media",
                                                                        style: "
                                                                            width: 100%;
                                                                            aspect-ratio: 1;
                                                                            object-fit: cover;
                                                                        ",
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        Some(Ok(_)) => {
                                            rsx! {
                                                div { style: "
                                                        padding: var(--space-4);
                                                        text-align: center;
                                                        color: var(--text-tertiary);
                                                        font-style: italic;
                                                    ",
                                                    "No similar media found. Try adjusting the threshold."
                                                }
                                            }
                                        }
                                        Some(Err(err)) => {
                                            rsx! {
                                                div { style: "
                                                        padding: var(--space-4);
                                                        text-align: center;
                                                        color: var(--error);
                                                    ",
                                                    "Error loading similar media: {err}"
                                                }
                                            }
                                        }
                                        None => {
                                            rsx! {
                                                div {
                                                    class: "similar-media-grid skeleton-grid",
                                                    style: "
                                                        display: grid;
                                                        grid-template-columns: repeat(3, 1fr);
                                                        gap: var(--space-2);
                                                    ",

                                                    for _ in 0..12 {
                                                        div {
                                                            class: "skeleton",
                                                            style: "
                                                                border-radius: var(--radius-md);
                                                                height: 100%;
                                                            ",
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Right column - All metadata, albums, and comments (scrollable)
                        div {
                            class: "media-detail-sidebar",
                            style: "
                                flex: 1;
                                display: flex;
                                flex-direction: column;
                                gap: var(--space-6);
                                overflow-y: auto;
                                max-height: calc(100vh - 140px);
                                padding-right: var(--space-2);
                            ",

                            // Media metadata form
                            div { class: "detail-section",
                                form {
                                    class: "media-detail-form",
                                    onsubmit: move |event| async move {
                                        let date = event.values().get("date").map(|v| v.as_value());
                                        let note = event.values().get("note").map(|v| v.as_value());
                                        match update_media(
                                                &UpdateMediaReq {
                                                    media_uuid,
                                                    update: MediaUpdate {
                                                        hidden: None,
                                                        date,
                                                        note,
                                                    },
                                                },
                                            )
                                            .await
                                        {
                                            Ok(_) => {
                                                status_signal.set("Changes saved successfully".to_string());
                                                update_signal.set(());
                                            }
                                            Err(err) => {
                                                status_signal.set(format!("Error: {}", err));
                                            }
                                        }
                                    },

                                    h2 { "Media Information" }

                                    div {
                                        class: "form-row",
                                        style: "display: flex; gap: var(--space-4);",
                                        div {
                                            class: "form-group",
                                            style: "flex: 1;",
                                            label { class: "form-label", "Library" }
                                            input {
                                                class: "form-input",
                                                r#type: "text",
                                                value: "{media.library_uuid}",
                                                disabled: true,
                                            }
                                        }

                                        div {
                                            class: "form-group",
                                            style: "flex: 1;",
                                            label { class: "form-label", "Path" }
                                            input {
                                                class: "form-input",
                                                r#type: "text",
                                                value: "{media.path}",
                                                disabled: true,
                                            }
                                        }
                                    }

                                    div {
                                        class: "form-row",
                                        style: "display: flex; gap: var(--space-4); align-items: flex-end;",
                                        div {
                                            class: "form-group",
                                            style: "flex: 1;",
                                            label { class: "form-label", "Date" }
                                            input {
                                                class: "form-input",
                                                name: "date",
                                                r#type: "text",
                                                value: "{date_formatted}",
                                            }
                                        }

                                        div {
                                            class: "form-group form-checkbox",
                                            style: "margin-bottom: var(--space-6);",
                                            input {
                                                id: "hidden-checkbox",
                                                r#type: "checkbox",
                                                checked: media.hidden,
                                                onclick: move |_| async move {
                                                    match update_media(
                                                            &UpdateMediaReq {
                                                                media_uuid,
                                                                update: MediaUpdate {
                                                                    hidden: Some(!media.hidden),
                                                                    date: None,
                                                                    note: None,
                                                                },
                                                            },
                                                        )
                                                        .await
                                                    {
                                                        Ok(_) => {
                                                            status_signal.set("Visibility updated".to_string());
                                                            update_signal.set(());
                                                        }
                                                        Err(err) => {
                                                            status_signal.set(format!("Error: {}", err));
                                                        }
                                                    }
                                                },
                                            }
                                            label { r#for: "hidden-checkbox", "Hidden" }
                                        }
                                    }

                                    div { class: "form-group",
                                        label { class: "form-label", "Note" }
                                        textarea {
                                            class: "form-textarea",
                                            name: "note",
                                            rows: 3,
                                            value: "{media.note}",
                                        }
                                    }

                                    div {
                                        class: "form-actions",
                                        style: "display: flex; align-items: center; gap: var(--space-4);",
                                        button {
                                            class: "btn btn-primary",
                                            r#type: "submit",
                                            "Save Changes"
                                        }

                                        button {
                                            class: "btn btn-secondary",
                                            onclick: move |_| {
                                                let link = full_link(media_uuid);
                                                let window = web_sys::window().expect("no global window exists");
                                                let _ = window.open_with_url_and_target(&link, "_blank");
                                            },
                                            "Download Original"
                                        }

                                        if !status_signal().is_empty() {
                                            span {
                                                class: "status-message",
                                                style: "color: var(--secondary);",
                                                "{status_signal()}"
                                            }
                                        }
                                    }
                                }
                            }

                            // Albums section - using our new component
                            AlbumDetailsTable {
                                album_uuids,
                                media_uuid,
                                update_signal,
                            }

                            // Use our new comments component
                            CommentsList {
                                comment_uuids,
                                media_uuid,
                                update_signal,
                            }

                            // Add some padding at the bottom to ensure good scrolling
                            div { style: "height: var(--space-4);" }
                        }
                    }
                }
            }
        }
        Some(Err(err)) => {
            rsx! {
                div { class: "container error-state",
                    h1 { "Error Loading Media" }
                    p { "There was an error loading the media: {err}" }
                    Link { to: Route::GallerySearch {}, class: "btn btn-primary", "Return to Gallery" }
                }
            }
        }
        None => {
            rsx! {
                div { class: "container loading-state",
                    div {
                        class: "skeleton",
                        style: "height: 40px; width: 200px; margin-bottom: 16px;",
                    }
                    div {
                        class: "media-detail-page skeleton-layout",
                        style: "display: flex; gap: var(--space-6);",

                        // Left column skeleton
                        div { style: "flex: 0 0 50%;",
                            div {
                                class: "skeleton",
                                style: "height: 400px; margin-bottom: 16px;",
                            }
                        }

                        // Right column skeleton
                        div { style: "flex: 1;",
                            div {
                                class: "skeleton",
                                style: "height: 24px; width: 80%; margin-bottom: 8px;",
                            }
                            div {
                                class: "skeleton",
                                style: "height: 24px; width: 60%; margin-bottom: 8px;",
                            }
                            div {
                                class: "skeleton",
                                style: "height: 100px; margin-bottom: 16px;",
                            }
                            div {
                                class: "skeleton",
                                style: "height: 150px; margin-bottom: 16px;",
                            }
                        }
                    }
                }
            }
        }
    }
}
