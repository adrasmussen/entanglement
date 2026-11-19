use dioxus::prelude::*;

use crate::common::slugs::{fetch_slugs, is_selected, pickable, selected, toggle};
use api::query::SearchKind;

// a picker of the values a field takes in the user's own data (GetSlugs), e.g. tags with how
// many visible records carry each.  the selection is written into `value` comma-joined, which
// the filter builder turns into one clause: a plain match for one value, any-of for more
#[component]
pub fn SlugPicker(
    kind: SearchKind,
    field: &'static str,
    multi: bool,
    value: Signal<String>,
    testid: &'static str,
) -> Element {
    let mut value = value;
    let mut filter = use_signal(String::new);

    // a new filter restarts the resource, dropping the pending (debounced) fetch
    let slugs = use_resource(move || async move {
        let prefix = filter().trim().to_lowercase();
        let mut list = fetch_slugs(kind, field, prefix, true).await;
        list.retain(|s| pickable(&s.value));
        list
    });

    let picked = value();
    let picked_list: Vec<String> = selected(&picked).iter().map(|s| s.to_string()).collect();

    rsx! {
        div { class: "slug-picker", "data-testid": testid,
            input {
                class: "form-input",
                "data-testid": "slug-filter",
                placeholder: "Filter values...",
                value: "{filter}",
                oninput: move |evt| filter.set(evt.value()),
            }
            if !picked_list.is_empty() {
                div { class: "slug-picked", "data-testid": "slug-picked",
                    for p in picked_list.iter() {
                        span { key: "{p}", class: "query-chip",
                            "{p}"
                            button {
                                r#type: "button",
                                class: "query-chip-remove",
                                "aria-label": "Remove {p}",
                                onclick: {
                                    let p = p.clone();
                                    move |_| {
                                        let next = toggle(&value.peek(), &p);
                                        value.set(next);
                                    }
                                },
                                "\u{d7}"
                            }
                        }
                    }
                }
            }
            div { class: "slug-options", "data-testid": "slug-options",
                match &*slugs.read() {
                    None => rsx! {
                        span { class: "slug-empty", "Loading values..." }
                    },
                    Some(list) if list.is_empty() => rsx! {
                        span { class: "slug-empty", "No values match" }
                    },
                    Some(list) => rsx! {
                        for s in list.iter() {
                            label { key: "{s.value}", class: "slug-option", "data-testid": "slug-option",
                                input {
                                    r#type: if multi { "checkbox" } else { "radio" },
                                    checked: is_selected(&picked, &s.value),
                                    onchange: {
                                        let v = s.value.clone();
                                        move |_| {
                                            let current = value.peek().clone();
                                            value.set(if multi { toggle(&current, &v) } else { v.clone() });
                                        }
                                    },
                                }
                                span { class: "slug-value", "{s.value}" }
                                if let Some(n) = s.count {
                                    span { class: "slug-count", "{n}" }
                                }
                            }
                        }
                    },
                }
            }
        }
    }
}
