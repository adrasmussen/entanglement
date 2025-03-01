use dioxus::prelude::*;

use crate::common::{
    local_time,
    modal::{MODAL_STACK, modal_err},
};
use api::album::*;

#[derive(Clone, PartialEq, Props)]
pub struct ShowAlbumBoxProps {
    album_uuid: AlbumUuid,
}

#[component]
pub fn ShowAlbumBox(props: ShowAlbumBoxProps) -> Element {
    let album_uuid = props.album_uuid;

    let status_signal = use_signal(|| String::from(""));

    let album = use_resource(move || async move {
        get_album(&GetAlbumReq {
            album_uuid: album_uuid,
        })
        .await
    });

    let album = match &*album.read() {
        Some(Ok(resp)) => resp.album.clone(),
        Some(Err(err)) => return modal_err(err.to_string()),
        None => return modal_err("Still waiting on get_album future..."),
    };

    rsx! {
        div { class: "modal-body",
            div {
                form { class: "modal-info",

                    label { "Name" }
                    span { "{album.name}" }

                    label { "Creator" }
                    span { "{album.uid}" }

                    label { "Group" }
                    span { "{album.gid}" }

                    label { "Note" }
                    span { "{album.note}" }

                    label { "Last modified" }
                    span { "{album.mtime}" }
                }
            }
            div { grid_column: "2",

                button { "Delete album" }
            }
        }
        div { class: "modal-footer",
            span { "{status_signal()}" }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct CreateAlbumBoxProps {}

#[component]
pub fn CreateAlbumBox() -> Element {
    let status_signal = use_signal(|| String::from(""));

    rsx! {
        div { class: "modal-body", grid_template_columns: "1fr",
            div {
                h3 { "Create album" }
                form {
                    onsubmit: move |event| async move {
                        let mut status_signal = status_signal;
                        let gid = match event.values().get("gid") {
                            Some(val) => val.as_value(),
                            None => String::from(""),
                        };
                        if gid.as_str() == "" {
                            status_signal.set(String::from("Error creating album: group is required"));
                            return;
                        }
                        let name = match event.values().get("name") {
                            Some(val) => val.as_value(),
                            None => String::from(""),
                        };
                        if name.as_str() == "" {
                            status_signal.set(String::from("Error creating album: name is required"));
                            return;
                        }
                        let note = match event.values().get("note") {
                            Some(val) => val.as_value(),
                            None => String::from(""),
                        };
                        let result = match add_album(
                                &AddAlbumReq {
                                    gid: gid,
                                    name: name,
                                    note: note,
                                },
                            )
                            .await
                        {
                            Ok(_) => {
                                MODAL_STACK.with_mut(|v| v.pop());
                                return;
                            }
                            Err(err) => format!("Error creating album: {}", err.to_string()),
                        };
                        status_signal.set(result)
                    },

                    p { "Group" }
                    input { name: "gid", r#type: "text", value: "" }

                    p { "Name" }
                    input { name: "name", r#type: "text", value: "" }

                    p { "Note" }
                    textarea {
                        name: "note",
                        rows: "8",
                        width: "100%",
                        value: "",
                    }

                    div { grid_template_columns: "1fr 1fr",
                        input { r#type: "submit", value: "Create album" }
                        button {
                            onclick: move |_| {
                                MODAL_STACK.with_mut(|v| v.pop());
                            },
                            "Cancel"
                        }
                    }
                }
            }
        }
        div { class: "modal-footer",
            span { "{status_signal}" }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct DeleteAlbumBoxProps {
    update_signal: Signal<()>,
    album_uuid: AlbumUuid,
}

#[component]
pub fn DeleteAlbumBox(props: DeleteAlbumBoxProps) -> Element {
    let mut update_signal = props.update_signal;
    let album_uuid = props.album_uuid;

    let status_signal = use_signal(|| String::from(""));

    let album = use_resource(move || async move {
        get_album(&GetAlbumReq {
            album_uuid: album_uuid,
        })
        .await
    });

    let album = &*album.read();

    let result = match album {
        Some(Ok(result)) => result.album.clone(),
        _ => {
            return rsx! {
                span { "error fetching {album_uuid}" }
            };
        }
    };

    let local_time = local_time(result.mtime);

    rsx! {
        div { class: "modal-body", grid_template_columns: "1fr",
            div {
                h3 { "Confirm album deletion" }
                p { "Owner: {result.uid}" }
                p { "Group: {result.gid}" }
                p { "Last modified: {local_time}" }
                p { white_space: "pre", "Note: {result.note}" }
                div { grid_template_columns: "1fr 1fr",
                    button {
                        onclick: move |_| async move {
                            let mut status_signal = status_signal;
                            match delete_album(
                                    &DeleteAlbumReq {
                                        album_uuid: album_uuid,
                                    },
                                )
                                .await
                            {
                                Ok(_) => {
                                    update_signal.set(());
                                    MODAL_STACK.with_mut(|v| v.pop());
                                    return;
                                }
                                Err(err) => {
                                    status_signal.set(format!("Error deleting album: {}", err.to_string()))
                                }
                            };
                        },
                        "Delete album"
                    }
                    button {
                        onclick: move |_| {
                            MODAL_STACK.with_mut(|v| v.pop());
                        },
                        "Cancel"
                    }
                }
            }
        }
        div { class: "modal-footer",
            span { "{status_signal}" }
        }
    }
}
