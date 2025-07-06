use dioxus::prelude::*;

use crate::{
    collection::card::CollectionCard,
    components::modal::{MODAL_STACK, Modal},
};
use api::collection::CollectionUuid;

#[derive(Clone, PartialEq, Props)]
pub struct CollectionGridProps {
    collections: Vec<CollectionUuid>,
}

#[component]
pub fn CollectionGrid(props: CollectionGridProps) -> Element {
    let has_collections = !props.collections.is_empty();

    rsx! {
        if has_collections {
            div {
                class: "collections-grid",
                style: "
                    display: grid;
                    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
                    gap: var(--space-4);
                    margin-top: var(--space-4);
                ",
                for collection_uuid in props.collections.iter() {
                    div { key: "{collection_uuid}",
                        CollectionCard { collection_uuid: *collection_uuid }
                    }
                }
            }
        } else {
            div {
                class: "empty-state",
                style: "
                    padding: var(--space-8) var(--space-4);
                    text-align: center;
                    background-color: var(--surface);
                    border-radius: var(--radius-lg);
                    margin-top: var(--space-4);
                ",
                div { style: "
                        font-size: 4rem;
                        margin-bottom: var(--space-4);
                        color: var(--neutral-400);
                    ",
                    "📂"
                }
                h3 { style: "
                        margin-bottom: var(--space-2);
                        color: var(--text-primary);
                    ",
                    "No Collections Found"
                }
                p { style: "
                        color: var(--text-secondary);
                        max-width: 500px;
                        margin: 0 auto;
                    ",
                    "No collections match your search criteria. Try adjusting your search or create a new collection to get started."
                }
                button {
                    class: "btn btn-primary",
                    style: "margin-top: var(--space-4);",
                    onclick: move |_| {
                        MODAL_STACK.with_mut(|v| v.push(Modal::CreateCollection));
                    },
                    "Create New Collection"
                }
            }
        }
    }
}
