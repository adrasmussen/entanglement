use std::collections::HashMap;

use anyhow;

use dioxus::prelude::*;

use api::{match_images, ImageFilter, MatchedImages};

pub fn Gallery() -> Element {
    let search_filter: Signal<ImageFilter> = use_signal(|| ImageFilter {
        filter: String::from(".*"),
    });

    // poke the api server
    let data = use_resource(|| async {gloo_net::http::Request::get("http://127.0.0.1:8081/api/img.json").send().await });

    let data = &*data.read();

    let data = match data {
        Some(d) => format!("{d:?}"),
        None => "got None response".to_owned()
    };

    // call to the api server
    let matching_images: Resource<anyhow::Result<MatchedImages>> =
        use_resource(move || async move { match_images(&search_filter()).await });

    // rebind to get around the issues with &*
    let matching_images = &*matching_images.read();

    let (images, status) = match matching_images {
        Some(Ok(matches)) => (Some(matches), "".to_owned()),
        Some(Err(err)) => (None, err.to_string()),
        None => (None, "still searching...".to_string()),
    };

    rsx! {
        div { "{data}" }
        div {
            match images {
                Some(images) => rsx! {
                    ul {
                        for (k, v) in images.images.iter() {
                            li {
                                key: "{k}",
                                img { src: "{v.url}"}
                            }
                        }

                    }
                },
                None => rsx! { p {"error finding images: {status}"} }
            }
        }
    }
}
