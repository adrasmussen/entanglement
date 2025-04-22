use std::cmp::Eq;
use std::future::Future;
use std::hash::Hash;

use dashmap::{mapref::entry::Entry, DashMap};

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
pub struct AwaitCache<K: Eq + Hash + Clone, V: Clone> {
    items: DashMap<K, V>,
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
        let val = match self.items.entry(key) {
            Entry::Occupied(entry) => entry.get().clone(),
            Entry::Vacant(entry) => {
                let val = init.await?;

                entry.insert(val.clone());
                val
            }
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
