use std::{cmp::Eq, fmt::Debug, future::Future, hash::Hash, sync::Arc};

use anyhow::Result;
use async_cell::sync::AsyncCell;
use dashmap::{mapref::entry::Entry, DashMap};
use tracing::{error, instrument};

pub mod auth;
pub mod config;
pub mod db;
pub mod media;

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

impl<K: Clone + Debug + Eq + Hash, V: Clone + Debug> AwaitCache<K, V> {
    pub fn new() -> Self {
        AwaitCache {
            items: DashMap::new(),
        }
    }

    #[instrument(skip(self, init))]
    pub async fn perhaps<Fut: Future<Output = Result<V>>>(&self, key: K, init: Fut) -> Result<V> {
        // since we need to determine if a thread should initialize the value in the map while
        // holding the lock, we can't use the native get() -- it would need to return Mutex<Option<>>
        // instead of the reverse
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
                    error!("error during cell initialization");
                    cell.set(None);
                    self.remove(&key);
                    return Err(anyhow::Error::from(err));
                }
            };

            cell.set(Some(val.clone()));

            val
        } else {
            cell.get().await.ok_or_else(|| anyhow::Error::msg("cell initializing thread failed"))?
        };

        Ok(val)
    }

    pub fn clear(&self) {
        self.items.clear();
    }

    pub fn remove(&self, key: &K) {
        self.items.remove(key);
    }
}
