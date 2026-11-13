// In-memory (lost on reload) cache of paged grids, so that coming back to a grid via the
// browser's back button renders everything that was loaded on the very first frame.  the
// router restores the scroll offset right after the first render, and that only lands in
// the right place if the document is already tall enough.
//
// nothing here touches the DOM or the network, which keeps the bookkeeping testable on the
// host.

use std::{
    any::Any,
    collections::{HashMap, VecDeque},
    rc::Rc,
};

use dioxus::prelude::*;

use api::{
    collection::{CollectionUuid, GetCollectionResp},
    library::{GetLibraryResp, LibraryUuid},
    media::MediaUuid,
    search::PAGE_SIZE,
};

use crate::components::media_grid::GRID_DENSITY;

// how many grids are remembered; the least recently used entry is evicted first
const MAX_ENTRIES: usize = 8;
// how many edits are remembered; a grid that is further behind than this refreshes
const MAX_EDITS: usize = 500;

pub type CacheKey = (&'static str, u64);

// windowing of the grid DOM (see MediaGrid): the cards before `start` are not mounted and
// are replaced by a spacer whose height is the sum of `spacers`, one entry per trimmed chunk
// (count of cards, measured height in px)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WindowState {
    pub start: usize,
    pub spacers: Vec<(usize, f64)>,
    // the layout the heights were measured in; they are meaningless in any other
    pub compact: bool,
    pub viewport: u32,
    // content width of the grid itself, 0.0 until the first resize event
    pub grid_width: f64,
}

impl WindowState {
    pub fn fresh(layout: LayoutKey) -> Self {
        Self {
            compact: layout.compact,
            viewport: layout.viewport,
            ..Default::default()
        }
    }

    pub fn spacer_px(&self) -> f64 {
        self.spacers.iter().map(|(_, px)| px).sum()
    }

    // unmount `count` more cards at the top, standing in for them with `px`
    pub fn trim(&mut self, count: usize, px: f64) {
        self.start += count;
        self.spacers.push((count, px));
    }

    // remount the most recently trimmed chunk
    pub fn untrim(&mut self) -> bool {
        match self.spacers.pop() {
            Some((count, _)) => {
                self.start -= count;
                true
            }
            None => false,
        }
    }

    // the state to use for a restored grid: the saved one if it was measured in the layout
    // we are in now, otherwise everything mounted (which is still full height)
    pub fn restored(self, layout: LayoutKey) -> Self {
        if self.compact == layout.compact && self.viewport == layout.viewport {
            self
        } else {
            Self::fresh(layout)
        }
    }
}

// the layout facts that decide whether measured heights are still valid
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutKey {
    pub compact: bool,
    pub viewport: u32,
}

impl LayoutKey {
    pub fn current() -> Self {
        let viewport = web_sys::window()
            .and_then(|w| w.inner_width().ok())
            .and_then(|v| v.as_f64())
            .unwrap_or_default() as u32;

        Self {
            compact: GRID_DENSITY.peek().is_compact(),
            viewport,
        }
    }
}

// how many cards one trim removes: whole rows, about a page worth, so that the cards that
// remain keep their columns
pub fn chunk_size(cols: usize) -> usize {
    let cols = cols.max(1);
    cols.max(PAGE_SIZE as usize / cols * cols)
}

#[derive(Clone)]
pub struct CacheEntry {
    // a Vec<T> for whatever T the grid pages (downcast on restore)
    pub items: Rc<dyn Any>,
    pub total: Option<u64>,
    pub exhausted: bool,
    // the EDIT_LOG sequence number this entry is up to date with
    pub edit_seq: u64,
    pub window: WindowState,
}

// most recently used last
#[derive(Default)]
pub struct GridCache {
    entries: Vec<(CacheKey, CacheEntry)>,
}

impl GridCache {
    pub fn get(&mut self, key: CacheKey) -> Option<CacheEntry> {
        let pos = self.entries.iter().position(|(k, _)| *k == key)?;
        let hit = self.entries.remove(pos);
        let entry = hit.1.clone();
        self.entries.push(hit);
        Some(entry)
    }

    pub fn put(&mut self, key: CacheKey, entry: CacheEntry) {
        self.entries.retain(|(k, _)| *k != key);
        self.entries.push((key, entry));
        if self.entries.len() > MAX_ENTRIES {
            self.entries.remove(0);
        }
    }

    // only the windowing part changes; entries that do not exist yet are left alone
    pub fn set_window(&mut self, key: CacheKey, window: WindowState) {
        if let Some((_, entry)) = self.entries.iter_mut().find(|(k, _)| *k == key) {
            entry.window = window;
        }
    }

    #[cfg(test)]
    fn keys(&self) -> Vec<CacheKey> {
        self.entries.iter().map(|(k, _)| *k).collect()
    }
}

pub enum EditsSince {
    // these media were edited since (deduplicated, possibly empty)
    Media(Vec<MediaUuid>),
    // more edits happened than are remembered
    Overflow,
}

// an append-only record of the media edited on the detail page.  it is a log with sequence
// numbers, not a set, so that every cached grid can catch up on its own
#[derive(Default)]
pub struct EditLog {
    next: u64,
    entries: VecDeque<(u64, MediaUuid)>,
}

impl EditLog {
    pub fn record(&mut self, uuid: MediaUuid) {
        self.entries.push_back((self.next, uuid));
        self.next += 1;
        while self.entries.len() > MAX_EDITS {
            self.entries.pop_front();
        }
    }

    // the sequence number a grid cached right now is up to date with
    pub fn seq(&self) -> u64 {
        self.next
    }

    pub fn since(&self, seq: u64) -> EditsSince {
        if seq >= self.next {
            return EditsSince::Media(Vec::new());
        }
        match self.entries.front() {
            Some((oldest, _)) if *oldest <= seq => {}
            _ => return EditsSince::Overflow,
        }

        let mut out = Vec::new();
        for (s, uuid) in &self.entries {
            if *s >= seq && !out.contains(uuid) {
                out.push(*uuid);
            }
        }
        EditsSince::Media(out)
    }
}

pub static GRID_CACHE: GlobalSignal<GridCache> = Signal::global(GridCache::default);
pub static MEDIA_EDITS: GlobalSignal<EditLog> = Signal::global(EditLog::default);

// header data of the detail pages, which otherwise gate a full page skeleton until their
// request resolves
pub static COLLECTION_META: GlobalSignal<HashMap<CollectionUuid, GetCollectionResp>> =
    Signal::global(HashMap::new);
pub static LIBRARY_META: GlobalSignal<HashMap<LibraryUuid, GetLibraryResp>> =
    Signal::global(HashMap::new);

// the library page's "show hidden" toggle is part of its grid's cache key, so it has to
// survive leaving the page too -- otherwise coming back from the hidden view restores the
// non-hidden list at the hidden view's scroll position
pub static LIBRARY_SHOW_HIDDEN: GlobalSignal<HashMap<LibraryUuid, bool>> =
    Signal::global(HashMap::new);

pub fn mark_media_edited(uuid: MediaUuid) {
    MEDIA_EDITS.write().record(uuid);
}

#[cfg(test)]
mod tests {
    use super::*;
    use api::UuidSource;

    struct Src;
    impl UuidSource for Src {}

    fn uuid(n: u8) -> MediaUuid {
        MediaUuid::try_parse(&Src, &format!("00000000-0000-0000-0000-0000000000{n:02}")).unwrap()
    }

    fn entry(tag: u32, seq: u64) -> CacheEntry {
        CacheEntry {
            items: Rc::new(vec![tag]),
            total: Some(1),
            exhausted: true,
            edit_seq: seq,
            window: WindowState::default(),
        }
    }

    fn tag(e: &CacheEntry) -> u32 {
        e.items.clone().downcast::<Vec<u32>>().unwrap()[0]
    }

    #[test]
    fn lru_evicts_least_recently_used() {
        let mut cache = GridCache::default();
        for i in 0..MAX_ENTRIES as u64 {
            cache.put(("g", i), entry(i as u32, 0));
        }
        // touching 0 makes 1 the oldest
        assert_eq!(tag(&cache.get(("g", 0)).unwrap()), 0);
        cache.put(("g", 100), entry(100, 0));

        assert!(cache.get(("g", 1)).is_none());
        assert!(cache.get(("g", 0)).is_some());
        assert!(cache.get(("g", 100)).is_some());
        assert_eq!(cache.keys().len(), MAX_ENTRIES);
    }

    #[test]
    fn put_replaces_same_key_and_scopes_are_distinct() {
        let mut cache = GridCache::default();
        cache.put(("a", 1), entry(1, 0));
        cache.put(("b", 1), entry(2, 0));
        cache.put(("a", 1), entry(3, 0));

        assert_eq!(cache.keys().len(), 2);
        assert_eq!(tag(&cache.get(("a", 1)).unwrap()), 3);
        assert_eq!(tag(&cache.get(("b", 1)).unwrap()), 2);
    }

    #[test]
    fn set_window_only_touches_existing_entries() {
        let mut cache = GridCache::default();
        let mut w = WindowState::default();
        w.trim(60, 100.0);

        cache.set_window(("a", 1), w.clone());
        assert!(cache.get(("a", 1)).is_none());

        cache.put(("a", 1), entry(1, 0));
        cache.set_window(("a", 1), w.clone());
        assert_eq!(cache.get(("a", 1)).unwrap().window, w);
    }

    #[test]
    fn edit_log_catch_up_is_independent_per_reader() {
        let mut log = EditLog::default();
        let gallery_seq = log.seq();
        log.record(uuid(1));
        let collection_seq = log.seq();
        log.record(uuid(2));
        log.record(uuid(1));

        match log.since(gallery_seq) {
            EditsSince::Media(v) => assert_eq!(v, vec![uuid(1), uuid(2)]),
            EditsSince::Overflow => panic!("unexpected overflow"),
        }
        match log.since(collection_seq) {
            EditsSince::Media(v) => assert_eq!(v, vec![uuid(2), uuid(1)]),
            EditsSince::Overflow => panic!("unexpected overflow"),
        }
        match log.since(log.seq()) {
            EditsSince::Media(v) => assert!(v.is_empty()),
            EditsSince::Overflow => panic!("unexpected overflow"),
        }
    }

    #[test]
    fn edit_log_overflows_when_behind_the_cap() {
        let mut log = EditLog::default();
        let old = log.seq();
        for _ in 0..MAX_EDITS + 5 {
            log.record(uuid(1));
        }
        assert!(matches!(log.since(old), EditsSince::Overflow));
        assert!(matches!(log.since(5), EditsSince::Media(_)));
        assert!(matches!(log.since(4), EditsSince::Overflow));
        assert_eq!(log.entries.len(), MAX_EDITS);
    }

    #[test]
    fn chunk_is_whole_rows_near_a_page() {
        assert_eq!(chunk_size(0), PAGE_SIZE as usize);
        assert_eq!(chunk_size(1), PAGE_SIZE as usize);
        assert_eq!(chunk_size(3), 60);
        assert_eq!(chunk_size(7), 56);
        assert_eq!(chunk_size(64), 64);
        for cols in 1..100 {
            let c = chunk_size(cols);
            assert_eq!(c % cols, 0);
            assert!(c >= cols);
        }
    }

    #[test]
    fn window_trim_and_untrim_are_inverse() {
        let mut w = WindowState::default();
        w.trim(60, 1000.5);
        w.trim(60, 999.5);
        assert_eq!(w.start, 120);
        assert_eq!(w.spacer_px(), 2000.0);

        assert!(w.untrim());
        assert_eq!((w.start, w.spacer_px()), (60, 1000.5));
        assert!(w.untrim());
        assert_eq!((w.start, w.spacer_px()), (0, 0.0));
        assert!(!w.untrim());
    }

    #[test]
    fn window_restore_requires_the_same_layout() {
        let layout = LayoutKey {
            compact: false,
            viewport: 1200,
        };
        let mut w = WindowState::fresh(layout);
        w.trim(60, 1000.0);

        assert_eq!(w.clone().restored(layout), w);

        let compact = LayoutKey {
            compact: true,
            ..layout
        };
        assert_eq!(w.clone().restored(compact), WindowState::fresh(compact));

        let narrow = LayoutKey {
            viewport: 800,
            ..layout
        };
        assert_eq!(w.restored(narrow), WindowState::fresh(narrow));
    }
}
