use std::collections::HashMap;

use anyhow;

use dioxus::prelude::*;

use api::{match_images, ImageFilter, MatchedImages};

#[derive(Clone, PartialEq, Props)]
pub struct ImageProps {
    sha: String,
    url: String,
}

pub fn Image(props: ImageProps) -> Element {
    rsx! {
        div {
            height: "400px",
            width: "400px",
            border: "5px solid #ffffff",
            display: "flex",
            flex_direction: "column",

            img {
                src: "{props.url}"
            }
        }
    }
}


pub fn Gallery() -> Element {
    let search_filter: Signal<ImageFilter> = use_signal(|| ImageFilter {
        filter: String::from(".*"),
    });

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
        div {
            match images {
                Some(images) => rsx! {
                    div {
                        display: "grid",
                        gap: "5px",
                        grid_template_columns: "repeat(auto-fit, minmax(400px, 1fr))",

                        for (k, v) in images.images.iter() {
                            Image { sha: "{k}", url: "{v.url}" }
                        }
                    }

                },
                None => rsx! { p {"error finding images: {status}"} }
            }
        }
    }
}
