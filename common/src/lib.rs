pub mod auth;
pub mod config;
pub mod db;

use std::cmp::Eq;
use std::future::Future;
use std::hash::Hash;

use async_cell::sync::AsyncCell;
use dashmap::{mapref::entry::Entry, DashMap};

pub struct AwaitCache<K: Eq + Hash + Clone, V: Clone> {
    items: DashMap<K, AsyncCell<V>>,
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
        match self.items.entry(key) {
            Entry::Occupied(entry) => Ok(entry.get().get().await),
            Entry::Vacant(entry) => {
                let val = init.await?;

                let cell = AsyncCell::new_with(val.clone());
                entry.insert(cell);
                Ok(val)
            }
        }
    }

    pub fn clear(&self) {
        self.items.clear();
    }

    pub fn remove(&self, key: &K) {
        self.items.remove(key);
    }
}
