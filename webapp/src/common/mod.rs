pub mod colors;
pub mod storage;
pub mod style;

use chrono::{Local, TimeZone};

pub fn local_time(secs: u64) -> String {
    let convert = move || {
        let secs = secs.try_into()?;

        let dt = Local
            .timestamp_opt(secs, 0)
            .single()
            .ok_or_else(|| anyhow::Error::msg(""))?;

        Result::<String, anyhow::Error>::Ok(dt.to_string())
    };
    match convert() {
        Ok(v) => v,
        Err(_) => String::from("error parsing timestamp"),
    }
}
