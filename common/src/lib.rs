use std::cmp::Eq;
use std::collections::HashSet;
use std::future::Future;
use std::hash::Hash;
use std::sync::Arc;

use anyhow::Result;
use async_cell::sync::AsyncCell;
use dashmap::{mapref::entry::Entry, DashMap};

pub mod auth;
pub mod config;
pub mod db;

// string validation
//
//
pub const USER_REGEX: &str = r"^[a-zA-Z0-9_.-]{1,64}$";
pub const GROUP_REGEX: &str = r"^[a-zA-Z0-9_.-]{1,64}$";
// awaitable cache
//
// this is loosely inspired by the WaitCache crate, except that we want to have requests await
// the result of the initial set operation instead of blocking.  for simplicity, we build it
// out of an AsyncCell, and just clone the contents instead of trying to manipulate the
// lifetimes of everything involved.
//
// since by construction, we write infrequently, it suffices to check that the things we are
// cloning (small HashSets) are not too big/costly
//
// it still uses a DashMap... which is problematic because there are weird issues holding the
// lock across an await, which is exactly what we do here with init.await
//
// before this is goes into production, we should check this carefully
pub struct AwaitCache<K: Eq + Hash + Clone, V: Clone> {
    items: DashMap<K, Arc<AsyncCell<V>>>,
}

impl<K: Eq + Hash + Clone, V: Clone> AwaitCache<K, V> {
    pub fn new() -> Self {
        AwaitCache {
            items: DashMap::new(),
        }
    }

    pub async fn perhaps<Fut: Future<Output = anyhow::Result<V>>>(
        &self,
        key: K,
        init: Fut,
    ) -> anyhow::Result<V> {
        let cell = match self.items.entry(key) {
            Entry::Occupied(entry) => entry.get().clone(),
            Entry::Vacant(entry) => {
                let val = init.await?;

                let cell = AsyncCell::new_with(val.clone());
                entry.insert(Arc::new(cell));
                return Ok(val);
            }
        };

        Ok(cell.get().await)
    }

    pub fn clear(&self) {
        self.items.clear();
    }

    pub fn remove(&self, key: &K) {
        self.items.remove(key);
    }
}
