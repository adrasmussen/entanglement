use anyhow;

use dioxus::prelude::*;

use api::media::*;

pub async fn search_media(req: &SearchMediaReq) -> anyhow::Result<SearchMediaResp> {
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

#[derive(Clone, PartialEq, Props)]
pub struct MediaProps {
    media_uuid: String,
}

#[component]
pub fn Media(props: MediaProps) -> Element {
    rsx! {
        div {
            height: "400px",
            width: "400px",
            border: "5px solid #ffffff",
            display: "flex",
            flex_direction: "column",

            img {
                src: "/entanglement/media/thumbnails/{props.media_uuid}",
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct GalleryNavBarProps {
    search_filter_signal: Signal<SearchMediaReq>,
    search_status: String,
}

#[component]
fn GalleryNavBar(props: GalleryNavBarProps) -> Element {
    let mut search_filter_signal = props.search_filter_signal.clone();

    let search_filter = search_filter_signal().filter;

    let style = r#"
        .subnav {
            overflow: hidden;
            background-color: #2196F3;
        }

        .subnav span {
            float: left;
            display: block;
            color: black;
            text-align: center;
            padding: 14px 16px;
            text-decoration: none;
            font-size: 17px;
        }

        .subnav span:hover {
            background-color: #eaeaea;
            color: black;
        }

        .subnav input[type=text], input[type=submit] {
            float: left;
            padding: 6px;
            border: none;
            margin-top: 8px;
            margin-right: 16px;
            margin-left: 6px;
            font-size: 17px;
  "#;

    // change this to a form and use onsubmit
    rsx! {
        div {
            style { "{style}" }
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
    let search_filter_signal: Signal<SearchMediaReq> = use_signal(|| SearchMediaReq {
        filter: String::from(".*"),
    });

    // call to the api server
    let search_results: Resource<anyhow::Result<SearchMediaResp>> =
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
        div {
            match results {
                Some(results) => rsx! {
                    div {
                        display: "grid",
                        gap: "5px",
                        grid_template_columns: "repeat(auto-fit, minmax(400px, 1fr))",

                        for m in results.media.iter() {
                            Media { media_uuid: "{m}" }
                        }
                    }

                },
                None => rsx! { p {""} }
            }
        }
    }
}
