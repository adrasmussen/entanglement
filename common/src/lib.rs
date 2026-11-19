use std::{
    cmp::Eq,
    fmt::Debug,
    future::Future,
    hash::Hash,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use async_cell::sync::AsyncCell;
use dashmap::{DashMap, mapref::entry::Entry};
use tracing::{error, instrument};

pub mod auth;
pub mod config;
pub mod db;
pub mod media;
pub mod server;

// entanglement common library
//
// this library exists to hold any logic that is shared between the server and other tools. it
// cannot be pulled into the webapp without significant feature-flagging and refactoring due to
// tokio not supporting a wasm build.
//
// the intent is that the cli tools should share as much of their code as possible with the
// server, since they exist to provide access to non-http api endpoints or to verify that some
// external service (i.e. LDAP) is functioning in an expected way.
pub fn unix_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time predates unix epoch")
        .as_secs()
}

// string validation
//
// we sanitize the inputs both to avoid awkward issues in the frontend as well as ensure that
// folding/unfolding sets of strings (see api/lib.rs) has no strange behavior
pub const USER_REGEX: &str = r"^[a-zA-Z0-9_.-]{1,64}$";
pub const GROUP_REGEX: &str = r"^[a-zA-Z0-9_.-]{1,64}$";

// awaitable cache
//
// this is loosely inspired by the WaitCache crate, except that we want to have requests await
// the result of the initial set operation instead of blocking.  for simplicity, we just clone
// the contents instead of trying to manipulate the lifetimes of everything involved.
//
// since by construction we write infrequently, it suffices to check that the things we are
// cloning (small HashSets) are not too big/costly
//
// note that there have historically been some issues with DashMap and holding references across
// await boundaries, but they have largely been cleared up
#[derive(Debug)]
pub struct AwaitCache<K: Clone + Debug + Eq + Hash, V: Clone + Debug> {
    items: DashMap<K, Arc<AsyncCell<Option<V>>>>,
}

impl<K: Clone + Debug + Eq + Hash, V: Clone + Debug> Default for AwaitCache<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Clone + Debug + Eq + Hash, V: Clone + Debug> AwaitCache<K, V> {
    pub fn new() -> Self {
        AwaitCache {
            items: DashMap::new(),
        }
    }

    #[instrument(skip(self, init))]
    pub async fn perhaps<Fut: Future<Output = Result<V>>>(&self, key: K, init: Fut) -> Result<V> {
        // since we need to determine if a thread should initialize the value in the map while
        // holding the lock, we can't use the native get() -- it would need to return Mutex<Option<V>>
        // instead of the transpose
        let (cell, set) = match self.items.entry(key.clone()) {
            Entry::Occupied(entry) => (entry.get().clone(), false),
            Entry::Vacant(entry) => {
                let cell = Arc::new(AsyncCell::new());

                entry.insert(cell.clone());
                (cell, true)
            }
        };

        // attempt to initialize the cell
        //
        // if this fails, we need to set the value to None (signalling to any listeners that this
        // attempt failed), remove the now-stale cell, and return an error to the caller
        //
        // we could instead have the Entry::Occupied match arm check if the value is None
        let val = if set {
            let val = match init.await {
                Ok(v) => v,
                Err(err) => {
                    error!("error during cell initialization: {err}");
                    cell.set(None);
                    self.remove(&key);
                    return Err(err);
                }
            };

            cell.set(Some(val.clone()));

            val
        } else {
            cell.get()
                .await
                .ok_or_else(|| anyhow::Error::msg("cell initializing thread failed"))?
        };

        Ok(val)
    }

    pub fn clear(&self) {
        self.items.clear();
    }

    pub fn remove(&self, key: &K) {
        self.items.remove(key);
    }

    // the keys present, including any still initializing
    pub fn keys(&self) -> Vec<K> {
        self.items.iter().map(|e| e.key().clone()).collect()
    }

    // the value for a key if one is set, without waiting on an initialization in progress
    pub fn peek(&self, key: &K) -> Option<V> {
        self.items
            .get(key)
            .and_then(|cell| cell.try_get().flatten())
    }

    // replace a key's value outright, e.g. with a fresher one from the source of truth;
    // later lookups get it without running their init
    pub fn insert(&self, key: K, val: V) {
        let cell = Arc::new(AsyncCell::new());
        cell.set(Some(val));
        self.items.insert(key, cell);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn await_cache_values_can_be_replaced_and_dropped() {
        let cache: AwaitCache<String, u32> = AwaitCache::new();
        let k = || "dev".to_string();

        assert_eq!(cache.perhaps(k(), async { Ok(1) }).await.unwrap(), 1);
        // a cached value is served without running init
        assert_eq!(cache.perhaps(k(), async { Ok(2) }).await.unwrap(), 1);
        assert_eq!(cache.keys(), vec![k()]);
        assert_eq!(cache.peek(&k()), Some(1));

        // a refresh replaces it outright
        cache.insert(k(), 3);
        assert_eq!(cache.peek(&k()), Some(3));
        assert_eq!(cache.perhaps(k(), async { Ok(4) }).await.unwrap(), 3);

        // once removed, the next lookup runs init again
        cache.remove(&k());
        assert_eq!(cache.peek(&k()), None);
        assert_eq!(cache.perhaps(k(), async { Ok(5) }).await.unwrap(), 5);
    }
}
