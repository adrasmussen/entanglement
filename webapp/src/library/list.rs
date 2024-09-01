use dioxus::prelude::*;

use crate::library::LibraryView;
use crate::common::style;
use api::library::*;

#[derive(Clone, PartialEq, Props)]
struct LibraryListEntryProps {
    library_view_signal: Signal<LibraryView>,
    library_uuid: LibraryUuid,
}

#[component]
fn LibraryListEntry(props: LibraryListEntryProps) -> Element {
    let mut library_view_signal = props.library_view_signal;
    let library_uuid = props.library_uuid;

    let libary = use_resource(move || async move {
        get_library(&GetLibraryReq {
            library_uuid: library_uuid,
        })
        .await
    });

    let library = &*libary.read();

    // this should throw a more informative error
    let result = match library {
        Some(Ok(result)) => result.library.clone(),
        _ => return rsx! {}
    };

    rsx! {
            tr {
                onclick: move |_| { library_view_signal.set(LibraryView::MediaList(library_uuid)) },
                td { "{result.path}" }
                td { "{result.gid}" }
                td { "{result.metadata.file_count}" }
                td { "{result.metadata.last_scan}" }
            }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct LibaryListProps {
    library_view_signal: Signal<LibraryView>,
    libraries: Vec<LibraryUuid>
}

#[component]
pub fn LibraryList(props: LibaryListProps) -> Element {
    rsx! {
        div {
            style { "{style::TABLE}" }
                table {
                    tr {
                        th { "Path" }
                        th { "Group" }
                        th { "File Count" }
                        th { "Last Scan" }
                    }

                    for library_uuid in props.libraries.iter() {
                        LibraryListEntry { library_view_signal: props.library_view_signal, library_uuid: *library_uuid }
                    }
                }
        }
    }
}
