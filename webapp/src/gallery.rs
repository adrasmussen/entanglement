use anyhow;

use dioxus::prelude::*;

use api::*;

pub async fn search_media(req: &SearchMediaReq) -> anyhow::Result<SearchMediaResp> {
    let resp: SearchMediaResp = gloo_net::http::Request::post("/api/search/image")
        .json(req)?
        .send()
        .await?
        .json()
        .await?;
    Ok(resp)
}

#[derive(Clone, PartialEq, Props)]
pub struct MediaProps {
    uuid: String,
    url: String,
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
                src: "{props.url}",
            }
        }
    }
}

#[component]
pub fn Gallery() -> Element {
    let search_filter: Signal<SearchMediaReq> = use_signal(|| SearchMediaReq {
        filter: String::from(".*"),
    });

    // call to the api server
    let matching_media: Resource<anyhow::Result<SearchMediaResp>> =
        use_resource(move || async move { search_media(&search_filter()).await });

    // rebind to get around the issues with &*
    let matching_media = &*matching_media.read();

    let (media, status) = match matching_media {
        Some(Ok(matches)) => (Some(matches), "".to_owned()),
        Some(Err(err)) => (None, err.to_string()),
        None => (None, "still searching...".to_string()),
    };

    rsx! {
        GalleryNavBar { search_filter_signal: search_filter }
        div {
            match media {
                Some(media) => rsx! {
                    div {
                        display: "grid",
                        gap: "5px",
                        grid_template_columns: "repeat(auto-fit, minmax(400px, 1fr))",

                        for m in media.media.iter() {
                            Media { uuid: "{m}", url: "http://localhost:8081/api/thumbnails/{m}.jpg" }
                        }
                    }

                },
                None => rsx! { p {"error finding images: {status}"} }
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct GalleryNavBarProps {
    search_filter_signal: Signal<SearchMediaReq>,
}

#[component]
fn GalleryNavBar(props: GalleryNavBarProps) -> Element {
    let mut signal = props.search_filter_signal.clone();
    let search_filter = &*signal.read().filter;

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

        .subnav input[type=text] {
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
                input {
                    r#type: "text",
                    value: "{search_filter}",
                    oninput: move |event| {
                        signal.set(SearchMediaReq{filter: event.value()})
                    }
                },
                span { "Search History" },
            }
        }
    }
}
