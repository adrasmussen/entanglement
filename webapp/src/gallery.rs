use anyhow;

use dioxus::prelude::*;

use crate::style;
use api::media::*;

// api functions

async fn search_media(req: &SearchMediaReq) -> anyhow::Result<SearchMediaResp> {
    let resp = gloo_net::http::Request::post("/entanglement/api/media")
        .json(&MediaMessage::SearchMedia(req.clone()))?
        .send()
        .await?;

    if resp.ok() {
        Ok(resp.json().await?)
    } else {
        Err(anyhow::Error::msg(resp.text().await?))
    }
}

async fn get_media(req: &GetMediaReq) -> anyhow::Result<GetMediaResp> {
    let resp = gloo_net::http::Request::post("/entanglement/api/media")
        .json(&MediaMessage::GetMedia(req.clone()))?
        .send()
        .await?;

    if resp.ok() {
        Ok(resp.json().await?)
    } else {
        Err(anyhow::Error::msg(resp.text().await?))
    }
}

#[derive(Clone, PartialEq, Props)]
struct MediaProps {
    view_media_signal: Signal<Option<MediaUuid>>,
    media_uuid: MediaUuid,
}

#[component]
fn Media(props: MediaProps) -> Element {
    let mut view_media_signal = props.view_media_signal.clone();
    let media_uuid = props.media_uuid.clone();

    rsx! {
        div {
            height: "400px",
            width: "400px",
            border: "5px solid #ffffff",
            display: "flex",
            flex_direction: "column",

            img {
                src: "/entanglement/media/thumbnails/{props.media_uuid}",
                onclick: move |_| { view_media_signal.set(Some(media_uuid)) }
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
struct MediaSidePanelProps {
    view_media_signal: Signal<Option<MediaUuid>>,
}

#[component]
fn MediaSidePanel(props: MediaSidePanelProps) -> Element {
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
        div {
            class: "sidepanel",

            match result {
                Some(result) => rsx!{
                    div {
                        img {
                            src: "/entanglement/media/thumbnails/{media_uuid}",
                        }
                        span { "library: {result.media.library_uuid}" },
                        span { "path: {result.media.path}" },
                        span { "hidden: {result.media.hidden}" },
                        span { "date: {result.media.metadata.date}" },
                        span { "note: {result.media.metadata.note}" },
                    }
                },
                None => rsx! {}
        }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
struct GalleryNavBarProps {
    search_filter_signal: Signal<SearchMediaReq>,
    search_status: String,
}

#[component]
fn GalleryNavBar(props: GalleryNavBarProps) -> Element {
    let mut search_filter_signal = props.search_filter_signal.clone();

    let search_filter = search_filter_signal().filter;

    rsx! {
        div {
            style { "{style::SUBNAV}" }
            div {
                class: "subnav",
                form {
                    onsubmit: move |event| {
                        let filter = match event.values().get("search_filter") {
                            Some(val) => val.as_value(),
                            None => String::from(""),
                        };
                        search_filter_signal.set(SearchMediaReq{filter: filter})
                    },
                    input {
                        name: "search_filter",
                        r#type: "text",
                        value: "{search_filter}",

                    },
                    input {
                        r#type: "submit"
                    },
                },
                span { "Search History" },
                span { "{props.search_status}" },
            }
        }
    }
}

#[component]
pub fn Gallery() -> Element {
    let search_filter_signal = use_signal(|| SearchMediaReq {
        filter: String::from(""),
    });

    let view_media_signal = use_signal::<Option<MediaUuid>>(|| None);

    // call to the api server
    let search_results =
        use_resource(move || async move { search_media(&search_filter_signal()).await });

    // annoying hack to get around the way the signals are implemented
    let search_results = &*search_results.read();

    let (results, status) = match search_results {
        Some(Ok(results)) => (
            Some(results),
            format!("found {} results", results.media.len()),
        ),
        Some(Err(err)) => (None, format!("error while searching: {}", err)),
        None => (None, format!("still awaiting search_media future...")),
    };

    rsx! {
        GalleryNavBar { search_filter_signal: search_filter_signal, search_status: status }
        MediaSidePanel { view_media_signal: view_media_signal }

        div {
            match results {
                Some(results) => rsx! {
                    div {
                        display: "grid",
                        gap: "5px",
                        grid_template_columns: "repeat(auto-fit, minmax(400px, 1fr))",

                        for media_uuid in results.media.iter() {
                            Media { view_media_signal: view_media_signal, media_uuid: *media_uuid }
                        }
                    }
                },
                None => rsx! { p {""} }
            }
        }
    }
}
