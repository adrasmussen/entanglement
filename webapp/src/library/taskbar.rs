use dioxus::prelude::*;

use crate::common::local_time;
use api::{library::*, task::*};

#[derive(Clone, PartialEq, Props)]
pub struct TaskBarProps {
    library_uuid: Memo<LibraryUuid>,
}

#[component]
pub fn TaskBar(props: TaskBarProps) -> Element {
    rsx! {
        div { style: "
            display: flex;
            gap: var(--space-4);
            margin-bottom: var(--space-3);
            color: var(--text-secondary);
            font-size: 0.875rem;
            ",

            ErrorBoundary {
                handle_error: |error: ErrorContext| {
                    rsx! {
                        if let Some(error_ui) = error.show() {
                            {error_ui}
                        } else {
                            span { "TaskBar encountered an error." }
                        }
                    }
                },
                TaskBarInner { library_uuid: props.library_uuid }
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
struct TaskBarInnerProps {
    library_uuid: Memo<LibraryUuid>,
}

#[component]
fn TaskBarInner(props: TaskBarInnerProps) -> Element {
    let library_uuid = props.library_uuid;

    // get information about any running tasks
    //
    // failures here shouldn't prevent the rest of the page from rendering
    let task_future = use_resource(move || async move {
        let library_uuid = library_uuid();

        show_tasks(&ShowTasksReq { library_uuid }).await
    });

    let task_data = &*task_future.read();
    let task_data = match task_data.clone().transpose().show(|error| {
        rsx! {
            span { "There was an error fetching tasks: {error}" }
        }
    })? {
        Some(v) => v,
        None => return rsx! {},
    };

    let tasks = task_data.tasks;

    match tasks.first() {
        None => {
            rsx! {
                span { "No recent tasks have run for this library." }
            }
        }
        Some(v) => {
            let start = local_time(v.start);
            let end = v.end.map(local_time).unwrap_or_else(|| "error".to_owned());
            let warnings = v.warnings.map(|i| i.to_string()).unwrap_or_else(|| "no".to_owned());

            match v.status {
                TaskStatus::Running => {
                    rsx! {
                        span { "Current task:" }
                        span { "{v.task_type} running, started by {v.uid} at {start}" }
                    }
                }
                _ => {
                    rsx! {
                        span { "Current task:" }
                        span { "{v.task_type} returned {v.status}, started by {v.uid} at {start} ended at {end} with {warnings} warnings" }
                    }
                }
            }
        }
    }
}
