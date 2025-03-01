pub mod auth;
pub mod config;
pub mod db;

use std::cmp::Eq;
use std::future::Future;
use std::hash::Hash;
use std::sync::Arc;

use async_cell::sync::AsyncCell;
use dashmap::{DashMap, mapref::entry::Entry};

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
