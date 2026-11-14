use std::marker::PhantomData;

use dioxus::prelude::*;

use crate::common::{
    query_ui::{
        Suggestion, accept, all_fields, append, chips, issue_line, op_label, placeholder,
        q_from_param, suggest, text_without_chip,
    },
    storage::{set_local_storage, try_local_storage},
};
use api::query::{SearchClass, SearchQuery};

// a page's search: the canonical query text in a signal (the grid cache keys and everything
// else read that string), its local storage key, and what to do on a commit besides storing
// it, which for the pages is replacing the URL's ?q=
#[derive(Clone, Copy, PartialEq)]
pub struct SearchState {
    pub signal: Signal<String>,
    pub storage_key: &'static str,
    pub on_commit: Callback<String>,
}

impl SearchState {
    // make `text` the page's search
    pub fn commit(mut self, text: String) {
        let text = text.trim().to_string();
        self.signal.set(text.clone());
        set_local_storage(self.storage_key, text.clone());
        self.on_commit.call(text);
    }

    pub fn text(&self) -> String {
        self.signal.peek().clone()
    }
}

// the search text of a page opened with the route's `q` parameter (still escaped, see
// q_param).  a non-empty `q` wins over the stored search and is stored in its turn; with none,
// the last search comes back from local storage
pub fn use_search_state(
    storage_key: &'static str,
    q: &str,
    on_commit: Callback<String>,
) -> SearchState {
    let from_url = q_from_param(q);

    let mut signal = use_signal({
        let from_url = from_url.clone();
        move || {
            if from_url.is_empty() {
                try_local_storage::<String>(storage_key)
            } else {
                set_local_storage(storage_key, from_url.clone());
                from_url
            }
        }
    });

    // the page stays mounted when only `q` changes (a link from elsewhere in the app)
    use_effect(use_reactive(&from_url, move |from_url| {
        if !from_url.is_empty() && from_url != *signal.peek() {
            set_local_storage(storage_key, from_url.clone());
            signal.set(from_url);
        }
    }));

    SearchState {
        signal,
        storage_key,
        on_commit,
    }
}

// the props of the generic components: a SearchState, and the class as a type parameter.
// Clone and PartialEq are written out because deriving them would demand them of the class
#[derive(Props)]
pub struct QueryBarProps<C: SearchClass + 'static> {
    search: SearchState,
    #[props(default)]
    status: String,
    #[props(default)]
    action_button: Option<Element>,
    #[props(default)]
    class: PhantomData<fn() -> C>,
}

impl<C: SearchClass + 'static> Clone for QueryBarProps<C> {
    fn clone(&self) -> Self {
        Self {
            search: self.search,
            status: self.status.clone(),
            action_button: self.action_button.clone(),
            class: PhantomData,
        }
    }
}

impl<C: SearchClass + 'static> PartialEq for QueryBarProps<C> {
    fn eq(&self, other: &Self) -> bool {
        self.search == other.search
            && self.status == other.status
            && self.action_button == other.action_button
    }
}

#[derive(Props)]
pub struct QueryChipsProps<C: SearchClass + 'static> {
    search: SearchState,
    #[props(default)]
    class: PhantomData<fn() -> C>,
}

impl<C: SearchClass + 'static> Clone for QueryChipsProps<C> {
    fn clone(&self) -> Self {
        Self {
            search: self.search,
            class: PhantomData,
        }
    }
}

impl<C: SearchClass + 'static> PartialEq for QueryChipsProps<C> {
    fn eq(&self, other: &Self) -> bool {
        self.search == other.search
    }
}

// the committed search as a row of chips, each with its field's help as a tooltip and an x
// that removes the clause
#[component]
pub fn QueryChips<C: SearchClass + 'static>(props: QueryChipsProps<C>) -> Element {
    let search = props.search;
    let parsed = SearchQuery::parse::<C>(&(search.signal)());

    rsx! {
        for chip in chips::<C>(&parsed.query) {
            span {
                class: "query-chip",
                title: "{chip.help}",
                "data-testid": "query-chip",
                span { class: "query-chip-label", "{chip.label}" }
                button {
                    class: "query-chip-remove",
                    r#type: "button",
                    "aria-label": "Remove {chip.label}",
                    "data-testid": "query-chip-remove",
                    onclick: move |_| {
                        search.commit(text_without_chip::<C>(&search.text(), chip.target));
                    },
                    "\u{d7}"
                }
            }
        }
    }
}

#[component]
pub fn QueryBar<C: SearchClass + 'static>(props: QueryBarProps<C>) -> Element {
    let search = props.search;
    let status = props.status.clone();

    let mut draft = use_signal(|| search.text());
    let mut focused = use_signal(|| false);
    let mut dismissed = use_signal(|| false);
    let mut active = use_signal(|| None::<usize>);
    let mut show_help = use_signal(|| false);
    // a double-click opens the list of every searchable field, whatever is typed
    let mut field_hints = use_signal(|| false);

    // chip edits, the filter builder and a new ?q= all change the committed text
    use_effect(move || {
        let committed = (search.signal)();
        draft.set(committed);
    });

    // the open list: every field after a double-click, otherwise completions for the typing
    let current_list = move |text: &str| -> Vec<Suggestion> {
        if *field_hints.peek() {
            all_fields::<C>()
        } else {
            suggest::<C>(text)
        }
    };

    let accept_at = use_callback(move |i: usize| {
        let text = draft.peek().clone();
        let hints = *field_hints.peek();
        if let Some(s) = current_list(&text).get(i) {
            draft.set(if hints {
                append(&text, s)
            } else {
                accept(&text, s)
            });
        }
        field_hints.set(false);
        active.set(None);
    });

    let committed = (search.signal)();
    let parsed = SearchQuery::parse::<C>(&committed);
    let issues: Vec<String> = parsed
        .issues
        .iter()
        .map(|i| issue_line(&committed, i))
        .collect();
    let has_chips = !parsed.query.is_empty();

    let suggestions = if focused() && !dismissed() {
        if field_hints() {
            all_fields::<C>()
        } else {
            suggest::<C>(&draft())
        }
    } else {
        Vec::new()
    };
    let open = !suggestions.is_empty();
    let placeholder = placeholder::<C>();

    rsx! {
        div { class: "search-bar query-bar",
            form {
                class: "query-form",
                onsubmit: move |_| {
                    // no prevent_default: in dioxus-web 0.6 forms never submit natively,
                    // and calling it on a submit event re-enables the native submission
                    dismissed.set(true);
                    search.commit(draft.peek().clone());
                },
                div { class: "search-input query-input-wrap",
                    input {
                        class: "form-input",
                        style: "width: 100%;",
                        name: "search_filter",
                        r#type: "text",
                        autocomplete: "off",
                        spellcheck: "false",
                        placeholder: "{placeholder}",
                        title: "{placeholder}",
                        "data-testid": "query-input",
                        value: "{draft}",
                        oninput: move |evt| {
                            draft.set(evt.value());
                            active.set(None);
                            dismissed.set(false);
                            field_hints.set(false);
                        },
                        ondoubleclick: move |_| {
                            field_hints.set(true);
                            dismissed.set(false);
                            active.set(None);
                        },
                        onfocus: move |_| focused.set(true),
                        onblur: move |_| {
                            focused.set(false);
                            field_hints.set(false);
                        },
                        onkeydown: move |evt| {
                            let n = current_list(&draft.peek()).len();
                            let open = *focused.peek() && !*dismissed.peek() && n > 0;
                            let current = *active.peek();
                            match evt.key() {
                                Key::ArrowDown if open => {
                                    evt.prevent_default();
                                    active.set(Some(current.map_or(0, |i| (i + 1) % n)));
                                }
                                Key::ArrowUp if open => {
                                    evt.prevent_default();
                                    active.set(Some(current.map_or(n - 1, |i| (i + n - 1) % n)));
                                }
                                Key::Enter if open && current.is_some() => {
                                    evt.prevent_default();
                                    accept_at.call(current.unwrap_or(0));
                                }
                                Key::Tab if open => {
                                    evt.prevent_default();
                                    accept_at.call(current.unwrap_or(0));
                                }
                                Key::Escape => {
                                    dismissed.set(true);
                                    field_hints.set(false);
                                }
                                _ => {}
                            }
                        },
                    }

                    if open {
                        ul { class: "query-suggestions", "data-testid": "query-suggestions",
                            for (i , s) in suggestions.iter().enumerate() {
                                li {
                                    key: "{s.insert}",
                                    class: if active() == Some(i) { "query-suggestion active" } else { "query-suggestion" },
                                    "data-testid": "query-suggestion",
                                    "data-insert": "{s.insert}",
                                    // mousedown, with the default prevented, keeps the input focused
                                    onmousedown: move |evt| {
                                        evt.prevent_default();
                                        accept_at.call(i);
                                    },
                                    span { class: "query-suggestion-title", "{s.title}" }
                                    span { class: "query-suggestion-help", "{s.help}" }
                                    if !s.example.is_empty() {
                                        code { class: "query-suggestion-example", "{s.example}" }
                                    }
                                }
                            }
                        }
                    }
                }
                button { class: "btn btn-primary", r#type: "submit", "Search" }
                button {
                    class: "btn btn-secondary query-help-toggle",
                    r#type: "button",
                    title: "What can I search for?",
                    "aria-expanded": "{show_help()}",
                    "data-testid": "query-help-toggle",
                    onclick: move |_| show_help.set(!show_help()),
                    "?"
                }
            }

            if let Some(action_button) = props.action_button {
                div { class: "search-actions", {action_button} }
            }

            if !status.is_empty() {
                span {
                    class: "search-status",
                    style: "margin-left: var(--space-4); color: var(--text-tertiary);",
                    "data-testid": "query-status",
                    "{status}"
                }
            }

            if show_help() {
                div { class: "query-help", "data-testid": "query-help",
                    p { class: "query-help-intro", "{C::FREE_TEXT_HELP}. Narrow it with field:value, e.g. {C::FIELDS.first().map(|f| f.example).unwrap_or(\"\")}." }
                    table { class: "query-help-table",
                        thead {
                            tr {
                                th { "Field" }
                                th { "Matches" }
                                th { "Help" }
                                th { "Example" }
                            }
                        }
                        tbody {
                            for f in C::FIELDS.iter() {
                                tr { key: "{f.name}",
                                    td { "{f.label}" }
                                    td { {f.ops.iter().map(|o| op_label(*o)).collect::<Vec<_>>().join(", ")} }
                                    td { "{f.help}" }
                                    td {
                                        code { "{f.example}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if has_chips {
                div { class: "query-chips", "data-testid": "query-chips",
                    QueryChips::<C> { search }
                }
            }

            if !issues.is_empty() {
                ul { class: "query-issues", "data-testid": "query-issues",
                    for (i , line) in issues.iter().enumerate() {
                        li {
                            key: "{i}",
                            class: "query-issue",
                            "data-testid": "query-issue",
                            "{line}"
                        }
                    }
                }
            }
        }
    }
}
