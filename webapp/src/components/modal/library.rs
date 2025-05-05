use dioxus::prelude::*;
use gloo_timers::callback::Timeout;

use crate::{
    common::local_time,
    components::modal::{ModalSize, ModernModal, MODAL_STACK},
};

use api::{library::LibraryUuid, task::*};

#[derive(Clone, PartialEq, Props)]
pub struct StartTaskModalProps {
    update_signal: Signal<()>,
    library_uuid: LibraryUuid,
}

#[component]
pub fn StartTaskModal(props: StartTaskModalProps) -> Element {
    let library_uuid = props.library_uuid;
    let mut update_signal = props.update_signal;
    let mut status_message = use_signal(String::new);
    let mut selected_task = use_signal(|| TaskType::ScanLibrary);
    let handle_submit = move |_| async move {
        status_message.set("Starting task...".into());
        match start_task(&StartTaskReq {
            library_uuid,
            task_type: selected_task(),
        })
        .await
        {
            Ok(_) => {
                status_message.set("Task started successfully".into());
                update_signal.set(());
                let timeout = Timeout::new(1500, move || {
                    MODAL_STACK.with_mut(|v| v.pop());
                });
                timeout.forget();
            }
            Err(err) => {
                status_message.set(format!("Error: {}", err));
            }
        }
    };
    let footer = rsx! {
        span { class: "status-message", style: "color: var(--primary);", "{status_message}" }
        div {
            class: "modal-buttons",
            style: "display: flex; gap: var(--space-4); justify-content: flex-end;",
            button {
                class: "btn btn-secondary",
                onclick: move |_| {
                    MODAL_STACK.with_mut(|v| v.pop());
                },
                "Cancel"
            }
            button { class: "btn btn-primary", onclick: handle_submit, "Start Task" }
        }
    };
    rsx! {
        ModernModal {
            title: "Start Library Task",
            size: ModalSize::Medium,
            footer,
            div { class: "task-selection-form",
                div { class: "form-group",
                    label { class: "form-label", "Select Task Type" }
                    div { class: "task-options",
                        TaskOption {
                            task_type: TaskType::ScanLibrary,
                            title: "Scan Library",
                            description: "Scan the library for new files and update the database.",
                            icon: "🔍",
                            is_selected: selected_task() == TaskType::ScanLibrary,
                            on_select: move |_| selected_task.set(TaskType::ScanLibrary),
                        }
                        TaskOption {
                            task_type: TaskType::CleanLibrary,
                            title: "Clean Library",
                            description: "Remove references to files that no longer exist in the filesystem.",
                            icon: "🧹",
                            is_selected: selected_task() == TaskType::CleanLibrary,
                            on_select: move |_| selected_task.set(TaskType::CleanLibrary),
                        }
                        TaskOption {
                            task_type: TaskType::RunScripts,
                            title: "Run Scripts",
                            description: "Execute maintenance and processing scripts on the library.",
                            icon: "📝",
                            is_selected: selected_task() == TaskType::RunScripts,
                            on_select: move |_| selected_task.set(TaskType::RunScripts),
                        }
                    }
                }
                div {
                    class: "task-details",
                    style: "margin-top: var(--space-6); padding: var(--space-4); background-color: var(--neutral-50); border-radius: var(--radius-md);",
                    h3 { style: "margin-bottom: var(--space-2); font-size: 1rem;",
                        "Task Details"
                    }
                    match selected_task() {
                        TaskType::ScanLibrary => rsx! {
                            p {
                                "This task will scan the library directory for new or modified files and update the database accordingly."
                            }
                            ul { style: "margin-top: var(--space-2); margin-left: var(--space-4); list-style-type: disc;",
                                li { "New files will be added to the database." }
                                li { "Thumbnails will be generated for new media." }
                                li { "Metadata will be extracted where possible." }
                                li { "Coming soon: scripts will run to update metadata and tags." }
                            }
                        },
                        TaskType::CleanLibrary => rsx! {
                            p {
                                "This task will check for database entries that no longer exist in the filesystem and mark them accordingly."
                            }
                            ul { style: "margin-top: var(--space-2); margin-left: var(--space-4); list-style-type: disc;",
                                li { "Missing symlinks to registered media will be recreated." }
                                li { "Removed originals will have their database entries and thumbnails removed." }
                                li { "References to missing files will be removed from collections." }
                                li { "No originals will be deleted from the filesystem." }
                            }
                        },
                        TaskType::RunScripts => rsx! {
                            p { "This task will execute any maintenance and processing scripts configured for this library." }
                            ul { style: "margin-top: var(--space-2); margin-left: var(--space-4); list-style-type: disc;",
                                li { "Custom metadata extraction may be performed." }
                                li { "Format conversions may run if configured." }
                                li { "Automated tagging may be performed." }
                            }
                        },
                    }
                    p { style: "margin-top: var(--space-3); font-style: italic; color: var(--text-tertiary);",
                        "Note: Tasks run in the background and you can continue using the application while they run.  For logs, contact the admins."
                    }
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
struct TaskOptionProps {
    task_type: TaskType,
    title: String,
    description: String,
    icon: String,
    is_selected: bool,
    on_select: EventHandler<MouseEvent>,
}

#[component]
fn TaskOption(props: TaskOptionProps) -> Element {
    let is_selected = props.is_selected;
    rsx! {
        div {
            class: if is_selected { "task-option selected" } else { "task-option" },
            onclick: move |evt| props.on_select.call(evt),
            div { class: "task-radio",
                div { class: "task-radio-outer",
                    if is_selected {
                        div { class: "task-radio-inner" }
                    }
                }
            }
            div { class: "task-icon", "{props.icon}" }
            div { class: "task-info",
                div { class: "task-name", "{props.title}" }
                div { class: "task-description", "{props.description}" }
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct StopTaskModalProps {
    update_signal: Signal<()>,
    library_uuid: LibraryUuid,
}

#[component]
pub fn StopTaskModal(props: StopTaskModalProps) -> Element {
    let library_uuid = props.library_uuid;

    let mut update_signal = props.update_signal;
    let mut status_message = use_signal(String::new);

    let task_future =
        use_resource(move || async move { show_tasks(&ShowTasksReq { library_uuid }).await });

    let tasks = &*task_future.read();

    let mut show_cancel_button = false;

    let modal_body = match tasks.clone().transpose() {
        Ok(Some(resp)) => match resp.tasks.first() {
            Some(task) => match task.status {
                TaskStatus::Running => {
                    show_cancel_button = true;
                    rsx! {
                        p { class: "confirmation-message", "Are you sure you want to stop this task?" }
                        div {
                            class: "media-info",
                            style: "margin-top: var(--space-4); padding: var(--space-3); background-color: var(--neutral-50); border-radius: var(--radius-md);",
                            p { "Type: {task.task_type}" }
                            p { "User: {task.uid}" }
                            p { "Start time: {local_time(task.start)}" }
                        }
                    }
                }
                _ => rsx! {
                    p { class: "confirmation-message", "No running task found" }
                },
            },
            None => rsx! {
                p { class: "confirmation-message", "Library has not run any tasks" }
            },
        },
        Ok(None) => rsx! {
            p { class: "confirmation-message", "future still resolving" }
        },
        Err(err) => rsx! {
            p { class: "confirmation-message", "Error fetching tasks: {err}" }
        },
    };

    let footer = rsx! {
        span { class: "status-message", "{status_message}" }
        div {
            class: "modal-buttons",
            style: "display: flex; gap: var(--space-4); justify-content: flex-end;",
            button {
                class: "btn btn-secondary",
                onclick: move |_| {
                    if !show_cancel_button {
                        update_signal.set(());
                    }
                    MODAL_STACK.with_mut(|v| v.pop());
                },
                "Cancel"
            }
            if show_cancel_button {
                button {
                    class: "btn btn-danger",
                    onclick: move |_| async move {
                        match stop_task(&StopTaskReq { library_uuid }).await {
                            Ok(_) => {
                                status_message.set("Task stopped".into());
                                update_signal.set(());
                                let timeout = Timeout::new(
                                    1500,
                                    move || {
                                        MODAL_STACK.with_mut(|v| v.pop());
                                    },
                                );
                                timeout.forget();
                            }
                            Err(err) => {
                                status_message.set(format!("Error: {}", err));
                            }
                        }
                    },
                    "Stop task"
                }
            }
        }
    };

    rsx! {
        ModernModal {
            title: "Confirm library task stop",
            size: ModalSize::Small,
            footer,
            div { class: "confirmation-content", {modal_body} }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct TaskHistoryModalProps {
    update_signal: Signal<()>,
    library_uuid: LibraryUuid,
}

#[component]
pub fn TaskHistoryModal(props: TaskHistoryModalProps) -> Element {
    let library_uuid = props.library_uuid;

    let task_history_future =
        use_resource(move || async move { show_tasks(&ShowTasksReq { library_uuid }).await });

    let status_signal = use_signal(String::new);

    let footer = rsx! {
        span { class: "status-message", "{status_signal}" }
        div {
            class: "modal-buttons",
            style: "display: flex; gap: var(--space-4); justify-content: flex-end;",
            button {
                class: "btn btn-primary",
                onclick: move |_| {
                    MODAL_STACK.with_mut(|v| v.pop());
                },
                "Close"
            }
        }
    };

    rsx! {
        ModernModal { title: "Task History", size: ModalSize::Large, footer,

            div {
                class: "task-history-content",
                style: "max-height: 60vh; overflow-y: auto;",
                match &*task_history_future.read() {
                    Some(Ok(response)) => {
                        if response.tasks.is_empty() {
                            rsx! {
                                div {
                                    class: "empty-state",
                                    style: "padding: var(--space-6); text-align: center; color: var(--text-tertiary);",
                                    "No task history available for this library."
                                }
                            }
                        } else {
                            rsx! {
                                div { class: "table-container", style: "position: relative;",
                                    table { style: "width: 100%; border-collapse: collapse; table-layout: fixed;",
                                        thead {
                                            tr {
                                                th { "Task Type" }
                                                th { "Status" }
                                                th { "User" }
                                                th { "Started" }
                                                th { "Completed" }
                                                th { "Warnings" }
                                            }
                                        }
                                        tbody {
                                            for (index , task) in response.tasks.iter().enumerate() {
                                                tr {
                                                    key: "{index}",
                                                    class: if index % 2 == 0 { "" } else { "row-alt" },
                                                    style: "border-bottom: 1px solid var(--border);",
                                                    td {
                                                        style: match task.status {
                                                            TaskStatus::Running => "font-weight: 600; color: var(--primary);",
                                                            _ => "",
                                                        },
                                                        "{task.task_type}"
                                                    }
                                                    td {
                                                        style: match task.status {
                                                            TaskStatus::Success => "color: var(--success);",
                                                            TaskStatus::Running => "color: var(--primary);",
                                                            TaskStatus::Failure | TaskStatus::Aborted => "color: var(--error);",
                                                            _ => "color: var(--text-tertiary);",
                                                        },
                                                        "{task.status}"
                                                    }
                                                    td { "{task.uid}" }
                                                    td { "{local_time(task.start)}" }
                                                    td {
                                                        if let Some(end_time) = task.end {
                                                            "{local_time(end_time)}"
                                                        } else {
                                                            "-"
                                                        }
                                                    }
                                                    td {
                                                        if let Some(warning_count) = task.warnings {
                                                            if warning_count > 0 {
                                                                span { style: "color: var(--warning);", "{warning_count}" }
                                                            } else {
                                                                span { "0" }
                                                            }
                                                        } else {
                                                            "-"
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                div { style: "margin-top: var(--space-4); font-size: 0.875rem; color: var(--text-tertiary);",
                                    "Showing {response.tasks.len()} task(s)"
                                }
                            }
                        }
                    }
                    Some(Err(err)) => rsx! {
                        div {
                            class: "error-state",
                            style: "padding: var(--space-4); color: var(--error); text-align: center;",
                            "Failed to load task history: {err}"
                        }
                    },
                    None => rsx! {
                        div { class: "loading-state",
                            for _ in 0..4 {
                                div { class: "skeleton", style: "height: 36px; margin-bottom: 8px;" }
                            }
                        }
                    },
                }
            }
        }
    }
}
