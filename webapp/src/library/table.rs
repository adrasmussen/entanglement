use dioxus::prelude::*;
use dioxus_router::prelude::*;

use crate::{Route, common::style};
use api::library::*;

#[derive(Clone, PartialEq, Props)]
struct LibraryTableRowProps {
    library_uuid: LibraryUuid,
}

#[component]
fn LibraryTableRow(props: LibraryTableRowProps) -> Element {
    let library_uuid = props.library_uuid;

    let library = use_resource(move || async move {
        get_library(&GetLibraryReq {
            library_uuid: library_uuid,
        })
        .await
    });

    let library = &*library.read();

    let result = match library {
        Some(Ok(result)) => result.library.clone(),
        _ => {
            return rsx! {
                tr {
                    span { "error fetching {library_uuid}" }
                }
            };
        }
    };

    rsx! {
        tr {
            td {
                Link {
                    to: Route::LibraryDetail {
                        library_uuid: library_uuid.to_string(),
                    },
                    span { "{result.path}" }
                }
            }
            td { "{result.gid}" }
            td { "{result.count}" }
            td { "{result.mtime}" }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct LibaryTableProps {
    libraries: Vec<LibraryUuid>,
}

#[component]
pub fn LibraryTable(props: LibaryTableProps) -> Element {
    rsx! {
        div {
            style { "{style::TABLE}" }
            table {
                tr {
                    th { "Path" }
                    th { "Group" }
                    th { "File count" }
                    th { "Last modified" }
                }

                for library_uuid in props.libraries.iter() {
                    LibraryTableRow { key: "{library_uuid}", library_uuid: *library_uuid }
                }
            }
        }
    }
}
