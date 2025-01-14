use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

use anyhow::Result;
use async_cell::sync::AsyncCell;
use tokio::sync::RwLock;

// NOTE TO SELF
//
// do we need both lock and cell here
pub struct LockMap<K, V> {
    inner: Arc<RwLock<HashMap<K, Arc<RwLock<AsyncCell<V>>>>>>,
}

impl<K: Eq + Hash + Clone, V> LockMap<K, V> {
    async fn get_cell_ref(&self, key: &K) -> Option<Arc<RwLock<AsyncCell<V>>>> {
        let map = self.inner.read().await;

        let cell = match map.get(key) {
            None => return None,
            Some(v) => v.clone(),
        };

        Some(cell)
    }

    async fn init_cell(&self, key: K) -> Result<()> {
        let mut map = self.inner.write().await;

        match map.insert(key.clone(), Arc::new(RwLock::new(AsyncCell::new()))) {
            None => return Ok(()),
            Some(val) => {
                map.insert(key, val);

                return Err(anyhow::Error::msg("key exists in lockmap"));
            }
        }
    }

    // we don't want to automatically create cells since the "empty cell" means that a thread
    // has picked up the work for that cell but hasn't finished filling it yet
    //
    // note that
    async fn set_cell(&self, key: &K, val: V) -> Result<()> {
        let cell = self
            .get_cell_ref(key)
            .await
            .ok_or_else(|| anyhow::Error::msg("key does not exist in lockmap"))?;

        let cell = cell.write().await;

        cell.set(val);

        Ok(())
    }
}
