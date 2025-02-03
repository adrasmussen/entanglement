pub mod modal;
pub mod storage;
pub mod stream;
pub mod style;

use chrono::{TimeZone, Local};

pub fn local_time(secs: i64) -> String {
    Local.timestamp_opt(secs, 0).single().map_or_else(
        || String::from("error parsing timestamp"),
        |dt| dt.to_string(),
    )
}
// preserved for reference later

/*
pub mod sidebar;
*/
