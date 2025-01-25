use dioxus::prelude::*;

use api::media::*;

use crate::common::style;

#[derive(Clone, PartialEq, Props)]
pub struct MediaSidePanelProps {
    view_media_signal: Signal<Option<MediaUuid>>,
}

#[component]
pub fn MediaSidePanel(props: MediaSidePanelProps) -> Element {
    let view_media_signal = props.view_media_signal.clone();

    let media_uuid = match view_media_signal() {
        Some(val) => val,
        None => return rsx! {},
    };

    let media = use_resource(move || async move {
        get_media(&GetMediaReq {
            media_uuid: media_uuid,
        })
        .await
    });

    let media = &*media.read();

    let (result, status) = match media {
        Some(Ok(res)) => (Some(res), format!("")),
        Some(Err(err)) => (None, err.to_string()),
        None => (None, format!("still awaiting get_media future...")),
    };

    rsx! {
        style { "{style::SIDEPANEL}" }
        div { class: "sidepanel",

            match result {
                Some(result) => rsx! {
                    div {
                        img { src: "/entanglement/media/full/{media_uuid}" }
                        span { "library: {result.media.library_uuid}" }
                        span { "path: {result.media.path}" }
                        span { "hidden: {result.media.hidden}" }
                        span { "date: {result.media.metadata.date}" }
                        span { "note: {result.media.metadata.note}" }
                    }
                },
                None => rsx! {},
            }
        }
    }
}
