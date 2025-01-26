pub mod auth;
pub mod config;
pub mod db;

use async_cell::sync::AsyncCell;
use dashmap::{mapref::entry::Entry, DashMap};
use std::cmp::Eq;
use std::hash::Hash;

pub struct AwaitCache<K: Clone + Eq + Hash, V: Clone> {
    items: DashMap<K, AsyncCell<V>>,
}

impl<K: Clone + Eq + Hash, V: Clone> AwaitCache<K, V> {
    pub fn new() -> Self {
        AwaitCache {
            items: DashMap::new(),
        }
    }

    pub async fn perhaps<F: FnOnce() -> V>(&self, key: K, init: F) -> V {
        match self.items.entry(key.clone()) {
            Entry::Occupied(entry) => entry.get().get().await,
            Entry::Vacant(entry) => {
                let val = init();
                let cell = AsyncCell::new_with(val.clone());
                entry.insert(cell);
                val
            }
        }
    }
}
