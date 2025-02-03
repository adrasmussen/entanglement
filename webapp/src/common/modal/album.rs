use dioxus::prelude::*;

use crate::common::modal::{modal_err, MODAL_STACK};
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
        div { class: "modal-body",
            div {
                form {
                    class: "modal-info",

                    onsubmit: move |event| async move {
                        let mut status_signal = status_signal;
                        let gid = match event.values().get("gid") {
                            Some(val) => val.as_value(),
                            None => {
                                status_signal
                                    .set(String::from("Error creating album: group is required"));
                                return;
                            }
                        };
                        let name = match event.values().get("name") {
                            Some(val) => val.as_value(),
                            None => {
                                status_signal
                                    .set(String::from("Error creating album: name is required"));
                                return;
                            }
                        };
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

                    label { "Group" }
                    input { name: "gid", r#type: "text", value: "" }

                    label { "Name" }
                    input { name: "name", r#type: "text", value: "" }

                    label { "Note" }
                    textarea { name: "note", rows: "8", value: "" }

                    input { r#type: "submit", value: "Create album" }
                }
            }
        }
        div { class: "modal-footer",
            span { "{status_signal()}" }
        }
    }
}
