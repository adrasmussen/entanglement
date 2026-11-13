use std::{
    cell::Cell,
    collections::HashSet,
    collections::hash_map::DefaultHasher,
    future::Future,
    hash::{Hash, Hasher},
    pin::Pin,
    rc::Rc,
};

use dioxus::prelude::*;

use crate::common::grid_cache::{
    CacheEntry, EditsSince, GRID_CACHE, LayoutKey, MEDIA_EDITS, WindowState,
};
use api::{
    WebError,
    collection::CollectionUuid,
    media::MediaUuid,
    search::{MAX_PAGE, PAGE_SIZE, SearchResponse},
};

// anything that can be paged needs a stable identity so that rows shifting between
// requests (inserts, deletes during scrolling) don't show up twice
pub trait PageItem: Clone + 'static {
    type Key: Clone + Eq + Hash;

    fn page_key(&self) -> Self::Key;

    // the media this item is, for the grids that can be edited through the detail page
    fn edit_uuid(&self) -> Option<MediaUuid> {
        None
    }
}

impl PageItem for SearchResponse {
    type Key = api::media::MediaUuid;

    fn page_key(&self) -> Self::Key {
        self.media_uuid
    }

    fn edit_uuid(&self) -> Option<MediaUuid> {
        Some(self.media_uuid)
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
    // which cards of `items` the grid has mounted (see MediaGrid)
    pub window: Signal<WindowState>,
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

// what the page makes of an item that was edited (on the detail page) while its grid was away
pub enum Reconcile<T> {
    // the item is still in the grid, just changed
    Patch(T),
    // the item no longer belongs in the grid
    Remove,
    // the change may have moved things around or changed membership, so re-fetch the head
    Refresh,
}

type ReconcileFuture<T> = Pin<Box<dyn Future<Output = Result<Reconcile<T>, WebError>>>>;

// opts a grid into the GRID_CACHE.  `scope` names the kind of grid (the cache key is the
// scope plus the hash of the reset key) and `reconcile(key, old_item)` decides what an edit
// of one of its items means for that grid
pub struct CacheCfg<K, T> {
    scope: &'static str,
    reconcile: Rc<dyn Fn(K, T) -> ReconcileFuture<T>>,
}

impl<K, T> CacheCfg<K, T> {
    pub fn new<F, Fut>(scope: &'static str, reconcile: F) -> Self
    where
        F: Fn(K, T) -> Fut + 'static,
        Fut: Future<Output = Result<Reconcile<T>, WebError>> + 'static,
    {
        Self {
            scope,
            reconcile: Rc::new(move |k, t| Box::pin(reconcile(k, t))),
        }
    }
}

// more edited items than this on a restore is not worth individual requests
const MAX_RECONCILE: usize = 10;

fn hash_key<K: Hash>(key: &K) -> u64 {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish()
}

// a grid restored from the cache
struct Restored<T> {
    items: Vec<T>,
    total: Option<u64>,
    exhausted: bool,
    edit_seq: u64,
    window: WindowState,
}

// Infinite-scroll paging.
//
// - `reset_key` is evaluated in an effect; when its value changes, everything is cleared
//   and the first page is fetched (and the window scrolls to the top)
// - `refresh` re-fetches the already-loaded prefix in place, keeping the scroll position
// - `cache` (optional) remembers the loaded items so that a remount with the same key
//   (browser back) renders them on its first frame without fetching anything
// - `fetch(key, offset, limit, with_total)` fetches one page
pub fn use_paged<T, K, F, Fut>(
    mut reset_key: impl FnMut() -> K + 'static,
    refresh: Signal<()>,
    cache: Option<CacheCfg<K, T>>,
    fetch: F,
) -> Paged<T>
where
    T: PageItem,
    K: Clone + Hash + PartialEq + 'static,
    F: Fn(K, u64, u64, bool) -> Fut + 'static,
    Fut: Future<Output = Result<Page<T>, WebError>> + 'static,
{
    let fetch = use_hook(|| Rc::new(fetch));
    let cache = use_hook(|| cache.map(Rc::new));
    let scope = cache.as_ref().map(|c| c.scope);

    // restore synchronously, before anything renders, so that the first frame is already
    // the full height grid
    let restored = use_hook(|| {
        let scope = scope?;
        let k = reset_key();
        let entry = GRID_CACHE.write().get((scope, hash_key(&k)))?;
        let items = entry.items.downcast::<Vec<T>>().ok()?;
        Some(Rc::new((
            k,
            Restored {
                items: (*items).clone(),
                total: entry.total,
                exhausted: entry.exhausted,
                edit_seq: entry.edit_seq,
                window: entry.window.restored(LayoutKey::current()),
            },
        )))
    });
    let restored_data = restored.as_deref().map(|(_, r)| r);

    let mut items = use_signal(|| restored_data.map(|r| r.items.clone()).unwrap_or_default());
    let mut total = use_signal(|| restored_data.and_then(|r| r.total));
    let mut loading = use_signal(|| false);
    let mut exhausted = use_signal(|| restored_data.map(|r| r.exhausted).unwrap_or(false));
    let mut initialized = use_signal(|| restored_data.is_some());
    let mut error = use_signal(|| None::<String>);
    let mut generation = use_signal(|| 0u64);
    let mut key = use_signal(|| restored.as_ref().map(|r| r.0.clone()));
    let mut window = use_signal(|| {
        restored_data
            .map(|r| r.window.clone())
            .unwrap_or_else(|| WindowState::fresh(LayoutKey::current()))
    });

    // write-through to the cache
    let store = use_callback(move |_: ()| {
        let Some(scope) = scope else {
            return;
        };
        let Some(k) = key.peek().clone() else {
            return;
        };
        GRID_CACHE.write().put(
            (scope, hash_key(&k)),
            CacheEntry {
                items: Rc::new(items.peek().clone()),
                total: *total.peek(),
                exhausted: *exhausted.peek(),
                edit_seq: MEDIA_EDITS.peek().seq(),
                window: window.peek().clone(),
            },
        );
    });

    // the windowing state is saved as it changes
    use_effect(move || {
        let w = window();
        let Some(scope) = scope else {
            return;
        };
        if let Some(k) = key.peek().as_ref() {
            GRID_CACHE.write().set_window((scope, hash_key(k)), w);
        }
    });

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
            window.set(WindowState::fresh(LayoutKey::current()));
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
                        store.call(());
                    }
                    Err(err) => error.set(Some(err.to_string())),
                }
                initialized.set(true);
                loading.set(false);
            });
        });
    }

    // re-fetch the loaded head in place, keeping whatever was loaded past it and the scroll
    // position
    let refresh_head = {
        let fetch = fetch.clone();
        use_callback(move |_: ()| {
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
                        let tail: Vec<T> =
                            items.peek().iter().skip(limit as usize).cloned().collect();
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

                        // the trimmed prefix is only still valid if the same items are in it
                        let start = window.peek().start;
                        if start > 0 {
                            let same = {
                                let old = items.peek();
                                old.len() >= start
                                    && list.len() >= start
                                    && old[..start]
                                        .iter()
                                        .zip(&list[..start])
                                        .all(|(a, b)| a.page_key() == b.page_key())
                            };
                            if !same {
                                window.set(WindowState::fresh(LayoutKey::current()));
                            }
                        }

                        items.set(list);
                        exhausted.set(done);
                        store.call(());
                    }
                    Err(err) => error.set(Some(err.to_string())),
                }
                initialized.set(true);
                loading.set(false);
            });
        })
    };

    // refresh in place (after modal edits etc.), skipping the initial run of the effect
    {
        let first_run = use_hook(|| Rc::new(Cell::new(true)));
        use_effect(move || {
            refresh();

            if first_run.replace(false) {
                return;
            }
            refresh_head.call(());
        });
    }

    // a restored grid that is behind the edit log gets just the edited items fixed up
    use_hook({
        let cache = cache.clone();
        let restored = restored.clone();
        move || {
            let (Some(cache), Some(restored)) = (cache, restored) else {
                return;
            };
            let (k, r) = &*restored;
            let (k, edit_seq) = (k.clone(), r.edit_seq);

            let edited = match MEDIA_EDITS.peek().since(edit_seq) {
                EditsSince::Media(v) if v.is_empty() => return,
                other => other,
            };

            spawn(async move {
                let EditsSince::Media(edited) = edited else {
                    refresh_head.call(());
                    return;
                };

                let present: Vec<T> = items
                    .peek()
                    .iter()
                    .filter(|i| i.edit_uuid().map(|u| edited.contains(&u)).unwrap_or(false))
                    .cloned()
                    .collect();

                if present.len() > MAX_RECONCILE {
                    refresh_head.call(());
                    return;
                }

                let mut needs_refresh = false;
                for old in present {
                    let uuid = old.page_key();
                    let outcome = match (cache.reconcile)(k.clone(), old).await {
                        Ok(outcome) => outcome,
                        Err(_) => Reconcile::Refresh,
                    };

                    match outcome {
                        Reconcile::Patch(new) => {
                            let mut list = items.write();
                            if let Some(slot) = list.iter_mut().find(|i| i.page_key() == uuid) {
                                *slot = new;
                            }
                        }
                        Reconcile::Remove => {
                            let removed_at = {
                                let mut list = items.write();
                                let pos = list.iter().position(|i| i.page_key() == uuid);
                                if let Some(pos) = pos {
                                    list.remove(pos);
                                }
                                pos
                            };
                            if let Some(pos) = removed_at {
                                total.with_mut(|t| {
                                    *t = t.map(|v| v.saturating_sub(1));
                                });
                                // an unmounted card was removed, so the spacer is wrong
                                if pos < window.peek().start {
                                    window.set(WindowState::fresh(LayoutKey::current()));
                                }
                            }
                        }
                        Reconcile::Refresh => needs_refresh = true,
                    }
                }

                if needs_refresh {
                    refresh_head.call(());
                } else {
                    store.call(());
                }
            });
        }
    });

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
                    let done = got < PAGE_SIZE || known_total.map(|t| len >= t).unwrap_or(false);
                    exhausted.set(done);
                    store.call(());
                }
                Err(err) => error.set(Some(err.to_string())),
            }
            initialized.set(true);
            loading.set(false);
        });
    });

    Paged {
        items,
        window,
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
