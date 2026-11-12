use std::{cell::Cell, collections::HashSet, future::Future, hash::Hash, rc::Rc};

use dioxus::prelude::*;

use api::{
    WebError,
    collection::CollectionUuid,
    search::{MAX_PAGE, PAGE_SIZE, SearchResponse},
};

// anything that can be paged needs a stable identity so that rows shifting between
// requests (inserts, deletes during scrolling) don't show up twice
pub trait PageItem: Clone + 'static {
    type Key: Clone + Eq + Hash;

    fn page_key(&self) -> Self::Key;
}

impl PageItem for SearchResponse {
    type Key = api::media::MediaUuid;

    fn page_key(&self) -> Self::Key {
        self.media_uuid
    }
}

impl PageItem for CollectionUuid {
    type Key = CollectionUuid;

    fn page_key(&self) -> Self::Key {
        *self
    }
}

// what a fetch callback returns
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: Option<u64>,
}

// the non-generic part of the paging state, which is what the sentinel and
// the status text need (and so can be a component prop)
#[derive(Clone, Copy, PartialEq)]
pub struct Pager {
    pub loading: Signal<bool>,
    pub exhausted: Signal<bool>,
    pub initialized: Signal<bool>,
    pub error: Signal<Option<String>>,
    pub total: Signal<Option<u64>>,
    pub load_more: Callback<()>,
}

pub struct Paged<T: 'static> {
    pub items: Signal<Vec<T>>,
    pub pager: Pager,
}

impl<T: 'static> Clone for Paged<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: 'static> Copy for Paged<T> {}

// append `new` to `old`, skipping anything already present
fn append_dedup<T: PageItem>(old: &mut Vec<T>, new: Vec<T>) {
    let mut seen: HashSet<T::Key> = old.iter().map(|i| i.page_key()).collect();
    for item in new {
        if seen.insert(item.page_key()) {
            old.push(item);
        }
    }
}

// Infinite-scroll paging.
//
// - `reset_key` is evaluated in an effect; when its value changes, everything is cleared
//   and the first page is fetched (and the window scrolls to the top)
// - `refresh` re-fetches the already-loaded prefix in place, keeping the scroll position
// - `fetch(key, offset, limit, with_total)` fetches one page
pub fn use_paged<T, K, F, Fut>(
    mut reset_key: impl FnMut() -> K + 'static,
    refresh: Signal<()>,
    fetch: F,
) -> Paged<T>
where
    T: PageItem,
    K: Clone + PartialEq + 'static,
    F: Fn(K, u64, u64, bool) -> Fut + 'static,
    Fut: Future<Output = Result<Page<T>, WebError>> + 'static,
{
    let fetch = use_hook(|| Rc::new(fetch));

    let mut items = use_signal(Vec::<T>::new);
    let mut total = use_signal(|| None::<u64>);
    let mut loading = use_signal(|| false);
    let mut exhausted = use_signal(|| false);
    let mut initialized = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut generation = use_signal(|| 0u64);
    let mut key = use_signal(|| None::<K>);

    // reset on key change
    {
        let fetch = fetch.clone();
        use_effect(move || {
            let new_key = reset_key();

            let previous = key.peek().clone();
            if previous.as_ref() == Some(&new_key) {
                return;
            }

            key.set(Some(new_key.clone()));
            let gen_id = {
                let mut g = generation.write();
                *g += 1;
                *g
            };
            items.write().clear();
            total.set(None);
            error.set(None);
            exhausted.set(false);
            initialized.set(false);
            loading.set(true);

            if previous.is_some() {
                let _ = document::eval("window.scrollTo({top: 0})");
            }

            let fetch = fetch.clone();
            spawn(async move {
                let res = fetch(new_key, 0, PAGE_SIZE, true).await;
                if *generation.peek() != gen_id {
                    return;
                }
                match res {
                    Ok(page) => {
                        let got = page.items.len() as u64;
                        let mut list = Vec::new();
                        append_dedup(&mut list, page.items);
                        let done = got < PAGE_SIZE
                            || page.total.map(|t| list.len() as u64 >= t).unwrap_or(false);
                        items.set(list);
                        total.set(page.total);
                        exhausted.set(done);
                    }
                    Err(err) => error.set(Some(err.to_string())),
                }
                initialized.set(true);
                loading.set(false);
            });
        });
    }

    // refresh in place (after modal edits etc.), skipping the initial run of the effect
    {
        let fetch = fetch.clone();
        let first_run = use_hook(|| Rc::new(Cell::new(true)));
        use_effect(move || {
            refresh();

            if first_run.replace(false) {
                return;
            }

            let Some(k) = key.peek().clone() else {
                return;
            };

            let gen_id = {
                let mut g = generation.write();
                *g += 1;
                *g
            };
            let loaded = items.peek().len() as u64;
            let limit = loaded.clamp(PAGE_SIZE, MAX_PAGE);
            loading.set(true);
            error.set(None);

            let fetch = fetch.clone();
            spawn(async move {
                let res = fetch(k, 0, limit, true).await;
                if *generation.peek() != gen_id {
                    return;
                }
                match res {
                    Ok(page) => {
                        let got = page.items.len() as u64;

                        // replace the head, keep whatever was loaded past it
                        let mut list = Vec::new();
                        append_dedup(&mut list, page.items);
                        let tail: Vec<T> = items.peek().iter().skip(limit as usize).cloned().collect();
                        append_dedup(&mut list, tail);

                        let mut done = if loaded <= limit {
                            got < limit
                        } else {
                            *exhausted.peek()
                        };
                        if let Some(t) = page.total {
                            total.set(Some(t));
                            if list.len() as u64 >= t {
                                done = true;
                            }
                        }
                        items.set(list);
                        exhausted.set(done);
                    }
                    Err(err) => error.set(Some(err.to_string())),
                }
                initialized.set(true);
                loading.set(false);
            });
        });
    }

    let load_more = use_callback(move |_: ()| {
        if *loading.peek() || *exhausted.peek() {
            return;
        }
        let Some(k) = key.peek().clone() else {
            return;
        };

        let gen_id = *generation.peek();
        let offset = items.peek().len() as u64;
        let with_total = offset == 0;
        error.set(None);
        loading.set(true);

        let fetch = fetch.clone();
        spawn(async move {
            let res = fetch(k, offset, PAGE_SIZE, with_total).await;
            if *generation.peek() != gen_id {
                return;
            }
            match res {
                Ok(page) => {
                    let got = page.items.len() as u64;
                    if let Some(t) = page.total {
                        total.set(Some(t));
                    }
                    let len = {
                        let mut list = items.write();
                        append_dedup(&mut list, page.items);
                        list.len() as u64
                    };
                    let known_total = page.total.or(*total.peek());
                    let done =
                        got < PAGE_SIZE || known_total.map(|t| len >= t).unwrap_or(false);
                    exhausted.set(done);
                }
                Err(err) => error.set(Some(err.to_string())),
            }
            initialized.set(true);
            loading.set(false);
        });
    });

    Paged {
        items,
        pager: Pager {
            loading,
            exhausted,
            initialized,
            error,
            total,
            load_more,
        },
    }
}

impl Pager {
    // "Showing X of Y things" / "Loading..." / "Error ..." for the search bar
    pub fn status(&self, loaded: usize, noun: &str) -> String {
        if let (Some(err), 0) = ((self.error)(), loaded) {
            return format!("Error searching {noun}: {err}");
        }
        if !(self.initialized)() {
            return String::from("Loading...");
        }
        match (self.total)() {
            Some(total) => format!("Showing {loaded} of {total} {noun}"),
            None => format!("Showing {loaded} {noun}"),
        }
    }
}
