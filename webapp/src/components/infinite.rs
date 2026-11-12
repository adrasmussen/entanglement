use dioxus::prelude::*;

use crate::common::paging::Pager;

#[derive(Clone, PartialEq, Props)]
pub struct InfiniteScrollSentinelProps {
    pager: Pager,
}

#[component]
pub fn InfiniteScrollSentinel(props: InfiniteScrollSentinelProps) -> Element {
    let pager = props.pager;
    let mut visible = use_signal(|| false);

    // load while the sentinel is on screen; this re-runs as loading flips back to
    // false, so tall viewports fill without any scrolling
    use_effect(move || {
        if visible()
            && (pager.initialized)()
            && !(pager.loading)()
            && !(pager.exhausted)()
            && (pager.error)().is_none()
        {
            pager.load_more.call(());
        }
    });

    let loading = (pager.loading)();
    let exhausted = (pager.exhausted)();
    let error = (pager.error)();

    rsx! {
        div {
            style: "display: flex; flex-direction: column; align-items: center; gap: var(--space-2); padding: var(--space-6) 0; min-height: 40px; color: var(--text-secondary);",
            onvisible: move |evt| {
                if let Ok(v) = evt.data().is_intersecting() {
                    visible.set(v);
                }
            },

            if let Some(err) = error {
                p { style: "color: var(--error); margin: 0;", "Error loading more: {err}" }
                button {
                    class: "btn btn-secondary",
                    onclick: move |_| pager.load_more.call(()),
                    "Retry"
                }
            } else if loading {
                p { "aria-live": "polite", style: "margin: 0;", "Loading more…" }
            } else if exhausted {
                p { style: "margin: 0; font-size: 0.75rem; color: var(--text-secondary);",
                    "End of results"
                }
            } else {
                button {
                    class: "btn btn-secondary",
                    onclick: move |_| pager.load_more.call(()),
                    "Load more"
                }
            }
        }
    }
}
