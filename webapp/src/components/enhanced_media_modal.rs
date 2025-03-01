use dioxus::prelude::*;

use crate::{
    common::{modal::MODAL_STACK, stream::full_link},
    components::modal::{ModernModal, modal_footer_buttons},
};
use api::media::*;

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

    let out = match &*media_future.read() {
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

    out
}
