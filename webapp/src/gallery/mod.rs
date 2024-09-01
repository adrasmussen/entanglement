use dioxus::prelude::*;

use crate::common::{
    modal::{Modal, ModalBox},
    storage::*,
    style,
};
use api::media::*;

pub mod grid;
use grid::MediaGrid;

// a compact way to describe the states the media page can be in
#[derive(Clone)]
enum GalleryView {
    Pending,
    MediaList(Vec<MediaUuid>),
    SearchError(String),
}

impl From<anyhow::Result<SearchMediaResp>> for GalleryView {
    fn from(smr: anyhow::Result<SearchMediaResp>) -> Self {
        match smr {
            Ok(resp) => GalleryView::MediaList(resp.media),
            Err(err) => GalleryView::SearchError(err.to_string()),
        }
    }
}

impl SearchStorage for SearchMediaReq {
    fn store(&self) -> () {
        set_local_storage("gallery_search_req", &self)
    }

    fn fetch() -> Self {
        match get_local_storage("gallery_search_req") {
            Ok(val) => val,
            Err(_) => Self::default()
        }
    }
}

#[derive(Clone, PartialEq, Props)]
struct GalleryNavBarProps {
    gallery_view_signal: Signal<GalleryView>,
}

#[component]
fn GalleryNavBar(props: GalleryNavBarProps) -> Element {
    let mut gallery_view_signal = props.gallery_view_signal;

    let search_filter = SearchMediaReq::fetch();

    let status = match gallery_view_signal() {
        GalleryView::Pending => String::from(""),
        GalleryView::MediaList(res) => format!("Found {} results", res.len()),
        GalleryView::SearchError(_) => String::from("Error while searching"),
    };

    rsx! {
        div {
            style { "{style::SUBNAV}" }
            div {
                class: "subnav",
                form {
                    onsubmit: move |event| async move {
                        let filter = match event.values().get("search_filter") {
                            Some(val) => val.as_value(),
                            None => String::from(""),
                        };

                        let req = SearchMediaReq{filter: filter};

                        gallery_view_signal.set(search_media(&req).await.into());

                        req.store();
                    },
                    input {
                        name: "search_filter",
                        r#type: "text",
                        value: "{search_filter.filter}",

                    },
                    input {
                        r#type: "submit",
                        value: "Search",
                    },
                },
                span { "Search History" },
                span { "{status}" },
            }
        }
    }
}

#[component]
pub fn Gallery() -> Element {
    let modal_stack_signal = use_signal::<Vec<Modal>>(|| Vec::new());
    let gallery_view_signal = use_signal(|| GalleryView::Pending);

    rsx! {
        GalleryNavBar { gallery_view_signal: gallery_view_signal }
        ModalBox { stack_signal: modal_stack_signal }

        match gallery_view_signal() {
            GalleryView::Pending => rsx! {
                span { "Search using the box above" }
            },
            GalleryView::MediaList(media) => rsx! {
                MediaGrid { modal_stack_signal: modal_stack_signal, media: media }
            },
            GalleryView::SearchError(err) => rsx! {
                span { "{err}" }
            }
        }
    }
}
