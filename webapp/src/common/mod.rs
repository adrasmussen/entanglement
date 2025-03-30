pub mod storage;
pub mod style;

use chrono::{Local, TimeZone};

pub fn local_time(secs: i64) -> String {
    Local.timestamp_opt(secs, 0).single().map_or_else(
        || String::from("error parsing timestamp"),
        |dt| dt.to_string(),
    )
}
