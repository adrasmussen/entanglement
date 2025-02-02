use dioxus::prelude::*;

use api::media::*;

#[derive(Clone, PartialEq, Props)]
pub struct MediaRelatedProps {
    media_uuid: MediaUuid,
    media: Media,
    status_signal: Signal<String>,
}

#[component]
pub fn MediaRelated(props: MediaRelatedProps) -> Element {
    rsx! {}
}
