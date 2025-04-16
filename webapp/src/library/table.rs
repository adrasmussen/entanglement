// webapp/src/library/table.rs

use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::{
    common::local_time,
    components::modal::{Modal, MODAL_STACK},
    Route,
};
use api::library::*;

#[derive(Clone, PartialEq, Props)]
pub struct LibraryTableProps {
    libraries: Vec<LibraryUuid>,
    update_signal: Signal<()>,
}

#[component]
pub fn LibraryTable(props: LibraryTableProps) -> Element {
    let libraries = props.libraries.clone();
    let _update_signal = props.update_signal;

    if libraries.is_empty() {
        return rsx! {
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
                    "📚"
                }
                h3 { style: "
                        margin-bottom: var(--space-2);
                        color: var(--text-primary);
                    ",
                    "No Libraries Found"
                }
                p { style: "
                        color: var(--text-secondary);
                        max-width: 500px;
                        margin: 0 auto;
                    ",
                    "No libraries match your search criteria. Try adjusting your search or contact an administrator to set up libraries."
                }
            }
        };
    }

    // Fetch details for each library
    let libraries_future = use_resource(move || {
        let libraries = libraries.clone();

        async move {
            let mut library_details = Vec::new();

            for library_uuid in libraries.iter() {
                match get_library(&GetLibraryReq {
                    library_uuid: *library_uuid,
                })
                .await
                {
                    Ok(resp) => library_details.push((*library_uuid, resp.library)),
                    Err(err) => {
                        tracing::error!("Failed to fetch library {library_uuid}: {err}");
                    }
                }
            }

            // Sort libraries by path for better display
            library_details.sort_by(|a, b| a.1.path.cmp(&b.1.path));
            library_details
        }
    });

    let libraries = &*libraries_future.read();

    let libraries = libraries.clone();

    // TODO -- reorganize actions/use tasks instead
    match libraries {
        Some(library_details) => {
            rsx! {
                div {
                    class: "table-container",
                    style: "
                        margin-top: var(--space-4);
                        background-color: var(--surface);
                        border-radius: var(--radius-lg);
                        overflow: hidden;
                        box-shadow: var(--shadow-sm);
                    ",
                    table { style: "width: 100%; border-collapse: collapse;",
                        thead {
                            tr { style: "background-color: var(--primary); color: white;",
                                th { style: "padding: var(--space-3); text-align: left;",
                                    "Path"
                                }
                                th { style: "padding: var(--space-3); text-align: left;",
                                    "Group"
                                }
                                th { style: "padding: var(--space-3); text-align: left;",
                                    "File Count"
                                }
                                th { style: "padding: var(--space-3); text-align: left;",
                                    "Last Modified"
                                }
                                th { style: "padding: var(--space-3); text-align: right;",
                                    "Actions"
                                }
                            }
                        }
                        tbody {
                            for (library_uuid , library) in library_details {
                                tr {
                                    key: "{library_uuid}",
                                    style: "border-bottom: 1px solid var(--border); transition: background-color var(--transition-fast) var(--easing-standard);",
                                    onmouseenter: move |_| {},

                                    // Path column with link
                                    td { style: "padding: var(--space-3);",
                                        Link {
                                            to: Route::LibraryDetail {
                                                library_uuid: library_uuid.to_string(),
                                            },
                                            style: "color: var(--primary); font-weight: 500; text-decoration: none;",
                                            "{library.path}"
                                        }
                                    }

                                    // Group column
                                    td { style: "padding: var(--space-3);",
                                        span {
                                            class: "group-badge",
                                            style: "
                                                display: inline-block;
                                                padding: var(--space-1) var(--space-2);
                                                background-color: var(--neutral-100);
                                                border-radius: var(--radius-full);
                                                font-size: 0.875rem;
                                            ",
                                            "{library.gid}"
                                        }
                                    }

                                    // File count column
                                    td { style: "padding: var(--space-3);", "{library.count}" }

                                    // Last modified column
                                    td { style: "padding: var(--space-3);",
                                        "{local_time(library.mtime)}"
                                    }

                                    // Actions column
                                    td { style: "padding: var(--space-3); text-align: right;",
                                        button {
                                            class: "btn btn-secondary",
                                            style: "margin-right: var(--space-2);",
                                            onclick: move |_| {
                                                MODAL_STACK.with_mut(|v| v.push(Modal::StartTask(library_uuid)));
                                            },
                                            "Start Task"
                                        }
                                        button {
                                            class: "btn btn-secondary",
                                            style: "margin-right: var(--space-2);",
                                            onclick: move |_| {
                                                tracing::info!("Scan library {library_uuid} clicked");
                                            },
                                            "Task History"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        None => {
            rsx! {
                div {
                    class: "loading-state",
                    style: "margin-top: var(--space-4); padding: var(--space-6);",
                    "Loading libraries..."
                }
            }
        }
    }
}
