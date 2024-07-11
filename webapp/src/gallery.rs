use anyhow;

use dioxus::prelude::*;

use api::image::{filter_images, FilterImageReq, FilterImageResp};

#[derive(Clone, PartialEq, Props)]
pub struct ImageProps {
    uuid: String,
    url: String,
}

#[component]
pub fn Image(props: ImageProps) -> Element {
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
    let search_filter: Signal<FilterImageReq> = use_signal(|| FilterImageReq {
        filter: String::from(".*"),
    });

    // call to the api server
    let matching_images: Resource<anyhow::Result<FilterImageResp>> =
        use_resource(move || async move { filter_images(&search_filter()).await });

    // rebind to get around the issues with &*
    let matching_images = &*matching_images.read();

    let (images, status) = match matching_images {
        Some(Ok(matches)) => (Some(matches), "".to_owned()),
        Some(Err(err)) => (None, err.to_string()),
        None => (None, "still searching...".to_string()),
    };

    rsx! {
        GalleryNavBar { search_filter_signal: search_filter }
        div {
            match images {
                Some(images) => rsx! {
                    div {
                        display: "grid",
                        gap: "5px",
                        grid_template_columns: "repeat(auto-fit, minmax(400px, 1fr))",

                        for (k, _) in images.images.iter() {
                            Image { uuid: "{k}", url: "http://localhost:8081/api/thumbnails/{k}.jpg" }
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
    search_filter_signal: Signal<FilterImageReq>
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
                        signal.set(FilterImageReq{filter: event.value()})
                    }
                },
                span { "Search History" },
            }
        }
    }
}
