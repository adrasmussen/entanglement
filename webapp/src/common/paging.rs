use std::{
    collections::HashSet,
    collections::hash_map::DefaultHasher,
    future::Future,
    hash::{Hash, Hasher},
    pin::Pin,
    rc::Rc,
};

use dioxus::prelude::*;
use futures_util::stream::{FuturesUnordered, StreamExt};
use tracing::warn;

use crate::common::grid_cache::{
    CacheEntry, CacheKey, EditsSince, GRID_CACHE, LayoutKey, MEDIA_EDITS, WindowState, reset_window,
};
use crate::common::hooks::use_on_write;
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
    // how many rows the page held on the server, when that can be more than items.len()
    // (see BatchSearchAndSortResp::page_len); None means items.len()
    pub page_len: Option<u64>,
}

impl<T> Page<T> {
    // how many rows the page held on the server, which is what decides whether it was the
    // last one (not to be confused with items.len())
    fn row_count(&self) -> u64 {
        self.page_len.unwrap_or(self.items.len() as u64)
    }
}

// where paging stands at the end of the loaded items
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Tail {
    // more can be loaded
    #[default]
    Open,
    // more can be loaded, but the last page added nothing and the loaded range was refreshed
    // because of it (see AfterLoad::Refresh); another such page gives up
    Stalled,
    // the end of the results
    Ended,
    // pages kept adding nothing even after a refresh, so loading stopped short of the end
    GaveUp,
}

impl Tail {
    pub fn can_load(self) -> bool {
        matches!(self, Self::Open | Self::Stalled)
    }

    // after a load_more page
    fn after_page(step: AfterLoad) -> Self {
        match step {
            AfterLoad::More => Self::Open,
            AfterLoad::Done => Self::Ended,
            AfterLoad::Refresh => Self::Stalled,
            AfterLoad::GiveUp => Self::GaveUp,
        }
    }

    // after the loaded range was re-fetched in place, `done` if that reached the end
    fn after_refresh(done: bool, cause: RefreshCause) -> Self {
        match (done, cause) {
            (true, _) => Self::Ended,
            (false, RefreshCause::Stall) => Self::Stalled,
            (false, RefreshCause::Requested) => Self::Open,
        }
    }

    // after a fetch failed: a stall is about pages that arrived, so it says nothing once one
    // didn't, and a closed tail would ignore the Retry that the error shows
    fn after_error(self) -> Self {
        match self {
            Self::Ended => Self::Ended,
            _ => Self::Open,
        }
    }
}

// the non-generic part of the paging state, which is what the sentinel and
// the status text need (and so can be a component prop)
#[derive(Clone, Copy, PartialEq)]
pub struct Pager {
    pub loading: Signal<bool>,
    pub tail: Signal<Tail>,
    pub initialized: Signal<bool>,
    pub error: Signal<Option<String>>,
    pub total: Signal<Option<u64>>,
    pub load_more: Callback<()>,
    // re-fetch everything loaded in place (which also gets a grid that gave up going again)
    pub refresh: Callback<()>,
}

pub struct Paged<T: 'static> {
    // shared with GRID_CACHE rather than copied into it (see PagedState::edit_items)
    pub items: Signal<Rc<Vec<T>>>,
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

// append `new` to `old`, skipping anything already present, and return how many were new
fn append_dedup<T: PageItem>(old: &mut Vec<T>, new: Vec<T>) -> usize {
    let before = old.len();
    let mut seen: HashSet<T::Key> = old.iter().map(|i| i.page_key()).collect();
    for item in new {
        if seen.insert(item.page_key()) {
            old.push(item);
        }
    }
    old.len() - before
}

// whether a page of `requested` rows that came back with `page_len` of them ends the results,
// with `loaded` items now in the grid
fn is_last_page(requested: u64, page_len: u64, loaded: usize, total: Option<u64>) -> bool {
    page_len < requested || total.is_some_and(|t| loaded as u64 >= t)
}

// whether the page about to load at `offset` should also re-count the results: always for
// the first page, and again once the loaded items reach the count we have, since rows added
// after it was taken would otherwise leave "Showing X of Y" with X past Y
fn needs_total(offset: u64, total: Option<u64>) -> bool {
    offset == 0 || total.is_some_and(|t| offset >= t)
}

// the count to show after a load_more page: a count that came with the page wins, and once
// the results have ended everything is loaded, so the count can't be below what's on screen
fn shown_total(stored: Option<u64>, fresh: Option<u64>, loaded: usize, done: bool) -> Option<u64> {
    let total = fresh.or(stored);
    if done {
        Some(total.unwrap_or(0).max(loaded as u64))
    } else {
        total
    }
}

// what load_more does after appending a page
#[derive(Clone, Copy, Debug, PartialEq)]
enum AfterLoad {
    // keep loading as the sentinel asks
    More,
    // that was the last page
    Done,
    // the page was nothing but items we already have, which happens when rows are inserted
    // ahead of the loaded range (offset paging); re-fetch the loaded range so that the next
    // offset is right again, instead of asking for the same page forever
    Refresh,
    // that already happened once and did not help
    GiveUp,
}

// `stalled` is whether a page added nothing since the last one that did
fn after_load(
    page_len: u64,
    added: usize,
    loaded: usize,
    total: Option<u64>,
    stalled: bool,
) -> AfterLoad {
    if is_last_page(PAGE_SIZE, page_len, loaded, total) {
        AfterLoad::Done
    } else if added > 0 {
        AfterLoad::More
    } else if stalled {
        AfterLoad::GiveUp
    } else {
        AfterLoad::Refresh
    }
}

// the (offset, limit) requests that re-fetch `loaded` items (at least a page) within MAX_PAGE
fn refresh_chunks(loaded: u64) -> Vec<(u64, u64)> {
    let target = loaded.max(PAGE_SIZE);
    (0..target)
        .step_by(MAX_PAGE as usize)
        .map(|offset| (offset, MAX_PAGE.min(target - offset)))
        .collect()
}

// why the loaded range is re-fetched.  only the refresh that a stall asked for keeps the
// stall on record, so that a second stall gives up instead of refreshing forever; any other
// refresh starts with a clean slate
#[derive(Clone, Copy, PartialEq)]
enum RefreshCause {
    Requested,
    Stall,
}

// the loaded range, as re-fetched by refetch()
struct Refetched<T> {
    items: Vec<T>,
    total: Option<u64>,
    done: bool,
}

// re-fetch the first `loaded` items, one MAX_PAGE chunk after the other, stopping early at the
// end of the results.  `current` is asked after every request, and None means that a newer
// generation took over in the meantime
async fn refetch<T, F, Fut>(
    loaded: u64,
    fetch: F,
    current: impl Fn() -> bool,
) -> Option<Result<Refetched<T>, WebError>>
where
    T: PageItem,
    F: Fn(u64, u64, bool) -> Fut,
    Fut: Future<Output = Result<Page<T>, WebError>>,
{
    let mut out = Refetched {
        items: Vec::new(),
        total: None,
        done: false,
    };

    for (offset, limit) in refresh_chunks(loaded) {
        let res = fetch(offset, limit, offset == 0).await;
        if !current() {
            return None;
        }
        let page = match res {
            Ok(page) => page,
            Err(err) => return Some(Err(err)),
        };

        if offset == 0 {
            out.total = page.total;
        }
        let page_len = page.row_count();
        append_dedup(&mut out.items, page.items);
        if is_last_page(limit, page_len, out.items.len(), out.total) {
            out.done = true;
            break;
        }
    }

    Some(Ok(out))
}

// whether the first `n` items of both lists are the same ones
fn same_prefix<T: PageItem>(old: &[T], new: &[T], n: usize) -> bool {
    old.len() >= n
        && new.len() >= n
        && old[..n]
            .iter()
            .zip(&new[..n])
            .all(|(a, b)| a.page_key() == b.page_key())
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

// the cached items that are among the `edited` media
fn edited_items<T: PageItem>(items: &[T], edited: &HashSet<MediaUuid>) -> Vec<T> {
    items
        .iter()
        .filter(|i| i.edit_uuid().is_some_and(|u| edited.contains(&u)))
        .cloned()
        .collect()
}

// one edited item, fixed up in place
enum Fix<T: PageItem> {
    Patch(T::Key, T),
    Remove(T::Key),
}

// what reconcile_edits makes of the edited items
enum Reconciled<T: PageItem> {
    // fix them up in place
    Apply(Vec<Fix<T>>),
    // at least one of them needs the loaded range re-fetched
    Refresh,
    // a refresh or reset took over in the meantime, and it has the edits covered
    Superseded,
}

// ask `reconcile` about all the edited items at once.  the first answer that needs a refresh
// (or fails) settles it, and the rest are dropped unanswered.  `current` is asked after
// every answer, as in refetch()
async fn reconcile_edits<T, F, Fut>(
    edited: Vec<T>,
    reconcile: F,
    current: impl Fn() -> bool,
) -> Reconciled<T>
where
    T: PageItem,
    F: Fn(T) -> Fut,
    Fut: Future<Output = Result<Reconcile<T>, WebError>>,
{
    let mut pending: FuturesUnordered<_> = edited
        .into_iter()
        .map(|old| {
            let key = old.page_key();
            let answer = reconcile(old);
            async move { (key, answer.await) }
        })
        .collect();

    let mut fixes = Vec::new();
    while let Some((key, res)) = pending.next().await {
        if !current() {
            return Reconciled::Superseded;
        }
        match res {
            Ok(Reconcile::Patch(new)) => fixes.push(Fix::Patch(key, new)),
            Ok(Reconcile::Remove) => fixes.push(Fix::Remove(key)),
            Ok(Reconcile::Refresh) => return Reconciled::Refresh,
            Err(err) => {
                warn!("failed to reconcile an edited item, refreshing instead: {err}");
                return Reconciled::Refresh;
            }
        }
    }
    Reconciled::Apply(fixes)
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

type PageFuture<T> = Pin<Box<dyn Future<Output = Result<Page<T>, WebError>>>>;
type FetchFn<K, T> = Rc<dyn Fn(K, u64, u64, bool) -> PageFuture<T>>;

// more edited items than this on a restore is not worth individual requests
const MAX_RECONCILE: usize = 10;

fn hash_key<K: Hash>(key: &K) -> u64 {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish()
}

// a grid restored from the cache, with the reset key it was stored under
struct Restored<K, T> {
    key: K,
    items: Rc<Vec<T>>,
    total: Option<u64>,
    tail: Tail,
    edit_seq: u64,
    window: WindowState,
}

// the cache entry for `key`, if there is one (of this kind of item)
fn restore<K: Hash, T: PageItem>(scope: &'static str, key: K) -> Option<Restored<K, T>> {
    let entry = GRID_CACHE.write().get((scope, hash_key(&key)))?;
    Some(Restored {
        key,
        items: entry.items.downcast::<Vec<T>>().ok()?,
        total: entry.total,
        tail: entry.tail,
        edit_seq: entry.edit_seq,
        window: entry.window.restored(LayoutKey::current()),
    })
}

// what is kept of a restore after the first render, for use_reconcile to catch up on the
// edits made since.  not the items: the signal and the cache are to be the only owners of
// the list (see PagedState::edit_items)
#[derive(Clone)]
struct RestoredFrom<K> {
    key: K,
    edit_seq: u64,
}

// the signals behind one paged grid.  every fetch belongs to a generation, and a reset or a
// refresh starts a new one, so that the results of an older fetch are dropped on arrival
struct PagedState<K: 'static, T: 'static> {
    scope: Option<&'static str>,
    key: Signal<Option<K>>,
    items: Signal<Rc<Vec<T>>>,
    window: Signal<WindowState>,
    total: Signal<Option<u64>>,
    loading: Signal<bool>,
    tail: Signal<Tail>,
    initialized: Signal<bool>,
    error: Signal<Option<String>>,
    generation: Signal<u64>,
    // the MEDIA_EDITS sequence number the items are known to be up to date with
    synced_seq: Signal<u64>,
}

impl<K: 'static, T: 'static> Clone for PagedState<K, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K: 'static, T: 'static> Copy for PagedState<K, T> {}

// the state, restored from the cache if possible.  that happens synchronously, before
// anything renders, so that the first frame is already the full height grid
fn use_paged_state<K: Clone + Hash + 'static, T: PageItem>(
    scope: Option<&'static str>,
    reset_key: &mut impl FnMut() -> K,
) -> (PagedState<K, T>, Option<RestoredFrom<K>>) {
    // only the first render restores, which is also the only one that runs the signal
    // initializers below
    let mut restored = None;
    let restored_from = use_hook(|| {
        restored = scope.and_then(|scope| restore::<K, T>(scope, reset_key()));
        restored.as_ref().map(|r| RestoredFrom {
            key: r.key.clone(),
            edit_seq: r.edit_seq,
        })
    });
    let restored = restored.as_ref();

    let state = PagedState {
        scope,
        key: use_signal(|| restored.map(|r| r.key.clone())),
        items: use_signal(|| restored.map(|r| r.items.clone()).unwrap_or_default()),
        window: use_signal(|| {
            restored
                .map(|r| r.window.clone())
                .unwrap_or_else(|| WindowState::fresh(LayoutKey::current()))
        }),
        total: use_signal(|| restored.and_then(|r| r.total)),
        loading: use_signal(|| false),
        tail: use_signal(|| restored.map(|r| r.tail).unwrap_or_default()),
        initialized: use_signal(|| restored.is_some()),
        error: use_signal(|| None),
        generation: use_signal(|| 0),
        synced_seq: use_signal(|| restored.map(|r| r.edit_seq).unwrap_or(0)),
    };
    (state, restored_from)
}

impl<K: Clone + Hash + 'static, T: PageItem> PagedState<K, T> {
    // start a fetch within the current generation
    fn begin(&mut self) -> u64 {
        self.error.set(None);
        self.loading.set(true);
        *self.generation.peek()
    }

    // start a fetch that supersedes everything in flight
    fn begin_generation(&mut self) -> u64 {
        *self.generation.write() += 1;
        self.begin()
    }

    fn is_current(&self, gen_id: u64) -> bool {
        *self.generation.peek() == gen_id
    }

    // the end of a fetch of generation `gen_id`: nothing at all if a newer generation took
    // over in the meantime, otherwise `apply` the result (if it is one) and settle.  returns
    // what `apply` returned
    fn finish<P, R>(
        &mut self,
        gen_id: u64,
        res: Result<P, WebError>,
        apply: impl FnOnce(&mut Self, P) -> R,
    ) -> Option<R> {
        if !self.is_current(gen_id) {
            return None;
        }
        match res {
            Ok(value) => {
                let out = apply(self, value);
                self.settle(Ok(()));
                Some(out)
            }
            Err(err) => {
                self.settle(Err(err));
                None
            }
        }
    }

    // the end of a fetch that is still current
    fn settle(&mut self, res: Result<(), WebError>) {
        if let Err(err) = res {
            self.error.set(Some(err.to_string()));
            let tail = self.tail.peek().after_error();
            self.tail.set(tail);
        }
        self.initialized.set(true);
        self.loading.set(false);
    }

    fn cache_key(&self) -> Option<CacheKey> {
        let scope = self.scope?;
        let key = self.key.peek();
        Some((scope, hash_key(key.as_ref()?)))
    }

    // write-through to the cache, which shares the item list instead of copying it
    fn store(&self) {
        let Some(cache_key) = self.cache_key() else {
            return;
        };
        GRID_CACHE.write().put(
            cache_key,
            CacheEntry {
                items: self.items.peek().clone(),
                total: *self.total.peek(),
                tail: *self.tail.peek(),
                edit_seq: *self.synced_seq.peek(),
                window: self.window.peek().clone(),
            },
        );
    }

    // change the items in place.  the cache entry shares the list (see store), and has to
    // let go of it first, or Rc::make_mut would copy the whole list; every caller stores
    // again right after
    fn edit_items<R>(&mut self, edit: impl FnOnce(&mut Vec<T>) -> R) -> R {
        if let Some(cache_key) = self.cache_key() {
            GRID_CACHE.write().remove(cache_key);
        }
        edit(Rc::make_mut(&mut self.items.write()))
    }

    // forget everything for a new key
    fn clear(&mut self, key: K) {
        self.key.set(Some(key));
        // a new list, since the cache may still have the old key's one
        self.items.set(Rc::default());
        reset_window(self.window);
        self.total.set(None);
        self.tail.set(Tail::Open);
        self.initialized.set(false);
    }

    // the first page after a reset; `seq` is the edit log position from before it was fetched
    fn set_first_page(&mut self, page: Page<T>, seq: u64) {
        let page_len = page.row_count();
        let mut list = Vec::new();
        append_dedup(&mut list, page.items);

        let last = is_last_page(PAGE_SIZE, page_len, list.len(), page.total);
        self.tail.set(if last { Tail::Ended } else { Tail::Open });
        self.items.set(Rc::new(list));
        self.total.set(page.total);
        self.synced_seq.set(seq);
        self.store();
    }

    // a load_more page
    fn append_page(&mut self, page: Page<T>) -> AfterLoad {
        // only a total that came with this very page can end the results: the stored one is
        // from the first page, and rows added since would otherwise end the results early
        let total = page.total;
        let page_len = page.row_count();
        let (added, loaded) = self.edit_items(|list| (append_dedup(list, page.items), list.len()));

        let stalled = *self.tail.peek() == Tail::Stalled;
        let step = after_load(page_len, added, loaded, total, stalled);
        let stored = *self.total.peek();
        self.total
            .set(shown_total(stored, total, loaded, step == AfterLoad::Done));
        match step {
            AfterLoad::Refresh => {
                warn!("a page of already loaded items at offset {loaded}, refreshing in place")
            }
            AfterLoad::GiveUp => {
                warn!("still no new items at offset {loaded} after a refresh, stopping here")
            }
            AfterLoad::More | AfterLoad::Done => {}
        }
        self.tail.set(Tail::after_page(step));
        self.store();

        step
    }

    // the whole loaded range, re-fetched in place
    fn replace_items(&mut self, fresh: Refetched<T>, seq: u64, cause: RefreshCause) {
        // the trimmed prefix is only still valid if the same items are in it
        let start = self.window.peek().start;
        if start > 0 && !same_prefix(&self.items.peek(), &fresh.items, start) {
            reset_window(self.window);
        }

        if let Some(t) = fresh.total {
            self.total.set(Some(t));
        }
        self.items.set(Rc::new(fresh.items));
        self.tail.set(Tail::after_refresh(fresh.done, cause));
        self.synced_seq.set(seq);
        self.store();
    }

    // see edit_items about storing afterwards
    fn apply_fix(&mut self, fix: Fix<T>) {
        match fix {
            Fix::Patch(key, new) => self.patch_item(&key, new),
            Fix::Remove(key) => self.remove_item(&key),
        }
    }

    fn patch_item(&mut self, key: &T::Key, new: T) {
        self.edit_items(|list| {
            if let Some(slot) = list.iter_mut().find(|i| i.page_key() == *key) {
                *slot = new;
            }
        });
    }

    fn remove_item(&mut self, key: &T::Key) {
        let removed_at = self.edit_items(|list| {
            let pos = list.iter().position(|i| i.page_key() == *key);
            if let Some(pos) = pos {
                list.remove(pos);
            }
            pos
        });
        if let Some(pos) = removed_at {
            self.total.with_mut(|t| {
                *t = t.map(|v| v.saturating_sub(1));
            });
            // an unmounted card was removed, so the spacer is wrong
            if pos < self.window.peek().start {
                reset_window(self.window);
            }
        }
    }
}

// the windowing state is saved as it changes
fn use_window_sync<K, T>(state: PagedState<K, T>)
where
    K: Clone + Hash + 'static,
    T: PageItem,
{
    use_effect(move || {
        let w = state.window.read().clone();
        let Some(scope) = state.scope else {
            return;
        };
        if let Some(k) = state.key.peek().as_ref() {
            GRID_CACHE.write().set_window((scope, hash_key(k)), w);
        }
    });
}

// clear everything and fetch the first page whenever the value of `reset_key` changes
fn use_reset_on_key<K, T>(
    state: PagedState<K, T>,
    mut reset_key: impl FnMut() -> K + 'static,
    fetch: FetchFn<K, T>,
) where
    K: Clone + Hash + PartialEq + 'static,
    T: PageItem,
{
    use_effect(move || {
        let new_key = reset_key();

        let previous = state.key.peek().clone();
        if previous.as_ref() == Some(&new_key) {
            return;
        }

        let mut state = state;
        let gen_id = state.begin_generation();
        state.clear(new_key.clone());

        if previous.is_some() {
            let _ = document::eval("window.scrollTo({top: 0})");
        }

        let seq = MEDIA_EDITS.peek().seq();
        let fetch = fetch.clone();
        spawn(async move {
            let res = fetch(new_key, 0, PAGE_SIZE, true).await;
            state.finish(gen_id, res, |s, page| s.set_first_page(page, seq));
        });
    });
}

// re-fetch the whole loaded range in place, keeping the scroll position
fn use_refresh<K, T>(state: PagedState<K, T>, fetch: FetchFn<K, T>) -> Callback<RefreshCause>
where
    K: Clone + Hash + 'static,
    T: PageItem,
{
    use_callback(move |cause: RefreshCause| {
        let Some(k) = state.key.peek().clone() else {
            return;
        };

        let mut state = state;
        let gen_id = state.begin_generation();
        let seq = MEDIA_EDITS.peek().seq();
        let loaded = state.items.peek().len() as u64;

        let fetch = fetch.clone();
        spawn(async move {
            let res = refetch(
                loaded,
                |offset, limit, with_total| fetch(k.clone(), offset, limit, with_total),
                || state.is_current(gen_id),
            )
            .await;
            // None: superseded between two of its requests
            if let Some(res) = res {
                state.finish(gen_id, res, |s, fresh| s.replace_items(fresh, seq, cause));
            }
        });
    })
}

// a restored grid that is behind the edit log gets just the edited items fixed up
fn use_reconcile<K, T>(
    state: PagedState<K, T>,
    cache: Option<Rc<CacheCfg<K, T>>>,
    restored: Option<RestoredFrom<K>>,
    refresh: Callback<RefreshCause>,
) where
    K: Clone + Hash + 'static,
    T: PageItem,
{
    use_hook(move || {
        let (Some(cache), Some(restored)) = (cache, restored) else {
            return;
        };

        let (edited, seq) = {
            let log = MEDIA_EDITS.peek();
            (log.since(restored.edit_seq), log.seq())
        };
        if matches!(&edited, EditsSince::Media(v) if v.is_empty()) {
            return;
        }

        let mut state = state;
        let key = restored.key.clone();
        let gen_id = *state.generation.peek();
        spawn(async move {
            let EditsSince::Media(edited) = edited else {
                refresh.call(RefreshCause::Requested);
                return;
            };

            let edited = edited_items(&state.items.peek(), &edited.into_iter().collect());
            if edited.len() > MAX_RECONCILE {
                refresh.call(RefreshCause::Requested);
                return;
            }

            let reconcile = |old| (cache.reconcile)(key.clone(), old);
            match reconcile_edits(edited, reconcile, || state.is_current(gen_id)).await {
                Reconciled::Apply(fixes) => {
                    for fix in fixes {
                        state.apply_fix(fix);
                    }
                    // only now are the edits accounted for in the cache
                    state.synced_seq.set(seq);
                    state.store();
                }
                Reconciled::Refresh => refresh.call(RefreshCause::Requested),
                Reconciled::Superseded => {}
            }
        });
    });
}

fn use_load_more<K, T>(
    state: PagedState<K, T>,
    fetch: FetchFn<K, T>,
    refresh: Callback<RefreshCause>,
) -> Callback<()>
where
    K: Clone + Hash + 'static,
    T: PageItem,
{
    use_callback(move |_: ()| {
        if *state.loading.peek() || !state.tail.peek().can_load() {
            return;
        }
        let Some(k) = state.key.peek().clone() else {
            return;
        };

        let mut state = state;
        let gen_id = state.begin();
        let offset = state.items.peek().len() as u64;
        // the first page (the Retry after a failed one) is one like after a reset, which
        // is what records the edit log position the items are up to date with
        let first = offset == 0;
        let with_total = needs_total(offset, *state.total.peek());
        let seq = MEDIA_EDITS.peek().seq();

        let fetch = fetch.clone();
        spawn(async move {
            let res = fetch(k, offset, PAGE_SIZE, with_total).await;
            let stalled = state.finish(gen_id, res, |s, page| {
                if first {
                    s.set_first_page(page, seq);
                    false
                } else {
                    s.append_page(page) == AfterLoad::Refresh
                }
            });

            // after settling, so that the refresh's loading flag is the one left standing
            if stalled == Some(true) {
                refresh.call(RefreshCause::Stall);
            }
        });
    })
}

// Infinite-scroll paging.
//
// - `reset_key` is evaluated in an effect; when its value changes, everything is cleared
//   and the first page is fetched (and the window scrolls to the top)
// - `refresh` re-fetches everything loaded in place, keeping the scroll position
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
    let fetch: FetchFn<K, T> = use_hook(|| {
        Rc::new(move |k, offset, limit, with_total| -> PageFuture<T> {
            Box::pin(fetch(k, offset, limit, with_total))
        })
    });
    let cache = use_hook(|| cache.map(Rc::new));
    let scope = cache.as_ref().map(|c| c.scope);

    let (state, restored) = use_paged_state(scope, &mut reset_key);

    use_window_sync(state);
    use_reset_on_key(state, reset_key, fetch.clone());
    let refresh_all = use_refresh(state, fetch.clone());
    // after modal edits etc.
    use_on_write(refresh, move || refresh_all.call(RefreshCause::Requested));
    use_reconcile(state, cache, restored, refresh_all);
    let load_more = use_load_more(state, fetch, refresh_all);
    let requested_refresh = use_callback(move |_: ()| refresh_all.call(RefreshCause::Requested));

    Paged {
        items: state.items,
        window: state.window,
        pager: Pager {
            loading: state.loading,
            tail: state.tail,
            initialized: state.initialized,
            error: state.error,
            total: state.total,
            load_more,
            refresh: requested_refresh,
        },
    }
}

impl Pager {
    // nothing more is going to load (reactive)
    pub fn exhausted(&self) -> bool {
        !(self.tail)().can_load()
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recounts_on_the_first_page_and_once_past_the_count() {
        assert!(needs_total(0, None));
        assert!(needs_total(0, Some(1500)));
        assert!(!needs_total(60, Some(1500)));
        assert!(!needs_total(1499, Some(1500)));
        assert!(needs_total(1500, Some(1500)));
        assert!(needs_total(1560, Some(1500)));
        // no count to go stale (e.g. a fetch that never returns one)
        assert!(!needs_total(60, None));
    }

    #[test]
    fn shown_total_prefers_a_fresh_count_and_never_trails_a_finished_list() {
        assert_eq!(shown_total(Some(1500), Some(1560), 1500, false), Some(1560));
        assert_eq!(shown_total(Some(1500), None, 1440, false), Some(1500));
        // finished past a stale count: everything is on screen
        assert_eq!(shown_total(Some(1500), None, 1530, true), Some(1530));
        // finished below the count (rows deleted meanwhile): keep the server's count
        assert_eq!(shown_total(Some(1500), None, 1490, true), Some(1500));
        assert_eq!(shown_total(None, None, 42, true), Some(42));
        assert_eq!(shown_total(None, None, 42, false), None);
    }
    use std::{
        cell::RefCell,
        future::ready,
        task::{Context, Poll, Waker},
    };

    // (key, version): the version stands in for an edit of the item
    #[derive(Clone, Copy, Debug, PartialEq)]
    struct It(u32, u32);

    impl PageItem for It {
        type Key = u32;

        fn page_key(&self) -> u32 {
            self.0
        }

        // the keys from 1000 on are media that can be edited
        fn edit_uuid(&self) -> Option<MediaUuid> {
            (self.0 >= 1000).then(|| uuid(self.0))
        }
    }

    struct Src;
    impl api::UuidSource for Src {}

    fn uuid(n: u32) -> MediaUuid {
        MediaUuid::try_parse(&Src, &format!("00000000-0000-0000-0000-{n:012}")).unwrap()
    }

    // the fetches below are always ready
    fn block_on<F: Future>(f: F) -> F::Output {
        let mut f = std::pin::pin!(f);
        let mut cx = Context::from_waker(Waker::noop());
        loop {
            if let Poll::Ready(v) = f.as_mut().poll(&mut cx) {
                return v;
            }
        }
    }

    // a server that pages `data` and records the requests it got
    fn server(
        data: &[It],
        requests: &RefCell<Vec<(u64, u64)>>,
    ) -> impl Fn(u64, u64, bool) -> std::future::Ready<Result<Page<It>, WebError>> {
        move |offset, limit, with_total| {
            requests.borrow_mut().push((offset, limit));
            let items: Vec<It> = data
                .iter()
                .skip(offset as usize)
                .take(limit as usize)
                .copied()
                .collect();
            ready(Ok(Page {
                total: with_total.then_some(data.len() as u64),
                page_len: Some(items.len() as u64),
                items,
            }))
        }
    }

    fn items(keys: std::ops::Range<u32>) -> Vec<It> {
        keys.map(|k| It(k, 0)).collect()
    }

    #[test]
    fn refresh_chunks_cover_the_loaded_range() {
        assert_eq!(refresh_chunks(0), vec![(0, PAGE_SIZE)]);
        assert_eq!(refresh_chunks(60), vec![(0, 60)]);
        assert_eq!(refresh_chunks(200), vec![(0, 200)]);
        assert_eq!(refresh_chunks(250), vec![(0, 200), (200, 50)]);
        assert_eq!(refresh_chunks(420), vec![(0, 200), (200, 200), (400, 20)]);
    }

    #[test]
    fn refetch_updates_items_past_max_page() {
        let mut data = items(0..1000);
        data[300].1 = 1;
        let requests = RefCell::new(Vec::new());

        let fresh = block_on(refetch(420, server(&data, &requests), || true))
            .unwrap()
            .unwrap();

        assert_eq!(fresh.items, data[..420]);
        assert_eq!(fresh.items[300], It(300, 1));
        assert_eq!(fresh.total, Some(1000));
        assert!(!fresh.done);
        assert_eq!(*requests.borrow(), vec![(0, 200), (200, 200), (400, 20)]);
    }

    #[test]
    fn refetch_after_a_head_insert_drops_nothing() {
        let old = items(0..250);
        let mut data = items(0..1000);
        data.insert(0, It(5000, 0));
        let requests = RefCell::new(Vec::new());

        let fresh = block_on(refetch(250, server(&data, &requests), || true))
            .unwrap()
            .unwrap();

        // the same loaded length, so the item pushed out of it is the next one load_more
        // asks for rather than lost between the head and an old tail
        assert_eq!(fresh.items.len(), 250);
        assert_eq!(fresh.items[0], It(5000, 0));
        let pushed_out: Vec<&It> = old.iter().filter(|o| !fresh.items.contains(o)).collect();
        assert_eq!(pushed_out, vec![&It(249, 0)]);
        assert_eq!(data[fresh.items.len()], It(249, 0));
    }

    #[test]
    fn refetch_stops_at_the_end_of_the_results() {
        let data = items(0..230);
        let requests = RefCell::new(Vec::new());

        let fresh = block_on(refetch(420, server(&data, &requests), || true))
            .unwrap()
            .unwrap();

        assert_eq!(fresh.items.len(), 230);
        assert!(fresh.done);
        assert_eq!(*requests.borrow(), vec![(0, 200), (200, 200)]);
    }

    #[test]
    fn refetch_gives_up_when_superseded() {
        let data = items(0..1000);
        let requests = RefCell::new(Vec::new());

        assert!(block_on(refetch(420, server(&data, &requests), || false)).is_none());
        assert_eq!(requests.borrow().len(), 1);
    }

    #[test]
    fn refetch_reports_errors() {
        let res = block_on(refetch(
            100,
            |_, _, _| ready(Err::<Page<It>, _>(WebError::msg("nope".into()))),
            || true,
        ));
        assert!(matches!(res, Some(Err(_))));
    }

    #[test]
    fn short_page_only_ends_the_results_by_its_server_length() {
        // media deleted before hydration: 55 items of a full 60 row page
        let page = Page {
            items: items(0..55),
            total: None,
            page_len: Some(60),
        };
        assert!(!is_last_page(PAGE_SIZE, page.row_count(), 55, None));

        // older servers do not say, so the item count is all there is
        let page = Page {
            items: items(0..55),
            total: None,
            page_len: None,
        };
        assert!(is_last_page(PAGE_SIZE, page.row_count(), 55, None));

        // a known total is reached
        assert!(is_last_page(PAGE_SIZE, 60, 120, Some(120)));
        assert!(!is_last_page(PAGE_SIZE, 60, 120, Some(121)));
    }

    #[test]
    fn a_page_of_known_items_refreshes_once_then_stops() {
        assert_eq!(after_load(60, 60, 120, None, false), AfterLoad::More);
        assert_eq!(after_load(60, 0, 120, None, false), AfterLoad::Refresh);
        assert_eq!(after_load(60, 0, 120, None, true), AfterLoad::GiveUp);
        // a page that adds something clears the stall (see append_page)
        assert_eq!(after_load(60, 1, 121, None, true), AfterLoad::More);
        // the end of the results wins over everything else
        assert_eq!(after_load(10, 0, 120, None, true), AfterLoad::Done);
        assert_eq!(after_load(60, 0, 120, Some(120), false), AfterLoad::Done);
    }

    #[test]
    fn a_full_page_without_a_total_is_not_the_end() {
        // load_more only asks for the total on the first page, so a later page past an old
        // total of 120 (rows were added since) comes without one and keeps going
        assert_eq!(after_load(60, 60, 120, None, false), AfterLoad::More);
        assert_eq!(after_load(60, 60, 180, None, false), AfterLoad::More);
        // whereas the stored total would have ended it
        assert_eq!(after_load(60, 60, 120, Some(120), false), AfterLoad::Done);
    }

    #[test]
    fn a_stall_refreshes_once_then_gives_up_unless_something_clears_it() {
        // a page of known items stalls, and the refresh that asks for keeps the stall
        let tail = Tail::after_page(after_load(60, 0, 120, None, false));
        assert_eq!(tail, Tail::Stalled);
        let tail = Tail::after_refresh(false, RefreshCause::Stall);
        assert_eq!(tail, Tail::Stalled);
        let step = after_load(60, 0, 120, None, tail == Tail::Stalled);
        assert_eq!(Tail::after_page(step), Tail::GaveUp);
        assert!(!Tail::GaveUp.can_load());

        // any other refresh clears it (and reopens a grid that gave up)
        assert_eq!(
            Tail::after_refresh(false, RefreshCause::Requested),
            Tail::Open
        );
        assert_eq!(Tail::after_refresh(true, RefreshCause::Stall), Tail::Ended);

        // as does an error, which also leaves the tail open for the Retry it shows
        assert_eq!(Tail::Stalled.after_error(), Tail::Open);
        assert_eq!(Tail::GaveUp.after_error(), Tail::Open);
        assert_eq!(Tail::Ended.after_error(), Tail::Ended);
        assert!(Tail::Stalled.can_load());
    }

    #[test]
    fn edited_items_are_the_loaded_edited_media() {
        let list = vec![It(1, 0), It(1000, 0), It(1001, 0), It(1002, 0)];
        let edited = HashSet::from([uuid(1), uuid(1001), uuid(1002), uuid(1005)]);
        // It(1) is not media, and 1005 is not loaded
        assert_eq!(edited_items(&list, &edited), vec![It(1001, 0), It(1002, 0)]);
        assert!(edited_items(&list, &HashSet::new()).is_empty());
    }

    // reconcile answers by key: 1 patches, 2 removes, 3 needs a refresh, 4 fails
    fn answer(old: It) -> std::future::Ready<Result<Reconcile<It>, WebError>> {
        ready(match old.0 {
            1 => Ok(Reconcile::Patch(It(1, old.1 + 1))),
            2 => Ok(Reconcile::Remove),
            3 => Ok(Reconcile::Refresh),
            _ => Err(WebError::msg("nope".into())),
        })
    }

    #[test]
    fn reconcile_edits_applies_patches_and_removals() {
        let res = block_on(reconcile_edits(vec![It(1, 0), It(2, 0)], answer, || true));
        let Reconciled::Apply(fixes) = res else {
            panic!("expected fixes");
        };
        let mut list = vec![It(0, 0), It(1, 0), It(2, 0)];
        for fix in fixes {
            match fix {
                Fix::Patch(k, new) => *list.iter_mut().find(|i| i.0 == k).unwrap() = new,
                Fix::Remove(k) => list.retain(|i| i.0 != k),
            }
        }
        assert_eq!(list, vec![It(0, 0), It(1, 1)]);
    }

    #[test]
    fn reconcile_edits_stops_at_a_refresh_or_an_error() {
        let res = block_on(reconcile_edits(vec![It(1, 0), It(3, 0)], answer, || true));
        assert!(matches!(res, Reconciled::Refresh));

        let res = block_on(reconcile_edits(vec![It(1, 0), It(4, 0)], answer, || true));
        assert!(matches!(res, Reconciled::Refresh));

        let res = block_on(reconcile_edits(vec![It(1, 0)], answer, || false));
        assert!(matches!(res, Reconciled::Superseded));

        let res = block_on(reconcile_edits(Vec::new(), answer, || true));
        assert!(matches!(res, Reconciled::Apply(f) if f.is_empty()));
    }

    #[test]
    fn append_dedup_counts_new_items() {
        let mut list = items(0..10);
        assert_eq!(append_dedup(&mut list, items(5..15)), 5);
        assert_eq!(append_dedup(&mut list, items(0..15)), 0);
        assert_eq!(list, items(0..15));
    }

    #[test]
    fn same_prefix_compares_keys_only() {
        let old = items(0..10);
        let mut new = items(0..10);
        new[3].1 = 7;
        assert!(same_prefix(&old, &new, 5));

        new.insert(0, It(99, 0));
        assert!(!same_prefix(&old, &new, 5));
        assert!(same_prefix(&old, &new, 0));
        assert!(!same_prefix(&old, &new[..3], 5));
    }
}
