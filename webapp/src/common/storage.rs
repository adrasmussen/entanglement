use anyhow;

use gloo_console::error as console_error;
use gloo_storage::{LocalStorage, Storage};

use serde::{Deserialize, Serialize};

pub fn set_local_storage<T>(key: &str, value: T) -> ()
where
    T: Serialize,
{
    let key = format!("entanglement_{}", key);

    LocalStorage::set(key.clone(), value)
        .unwrap_or_else(|err| console_error!(format!("Failed to set local storage {key}: {err}")))
}

pub fn get_local_storage<T>(key: &str) -> anyhow::Result<T>
where
    T: for<'a> Deserialize<'a>,
{
    let key = format!("entanglement_{}", key);

    LocalStorage::get(key.clone()).map_err(|err| {
        console_error!(format!("Failed to fetch local storage {key}: {err}"));
        anyhow::Error::msg("Local storage failure, see console log")
    })
}

pub fn try_local_storage<T>(key: &str) -> T
where
    T: for<'a> Deserialize<'a> + Default,
{
    let key = format!("entanglement_{}", key);

    match LocalStorage::get(key.clone()) {
        Ok(val) => val,
        Err(_) => T::default(),
    }
}

pub fn try_and_forget_local_storage<T>(key: &str) -> T
where
    T: Serialize + for<'a> Deserialize<'a> + Default,
{
    let key = format!("entanglement_{}", key);

    match LocalStorage::get(key.clone()) {
        Ok(val) => {
            set_local_storage(&key, T::default());
            val
        }
        Err(_) => T::default(),
    }
}

pub trait SearchStorage
where
    Self: Serialize,
    Self: for<'a> Deserialize<'a>,
{
    fn store(&self) -> ();

    fn fetch() -> Self;
}
