pub mod image;
pub mod auth;

pub type MediaUuid = u64;

pub struct PartialDate {
    year: Option<String>,
    month: Option<String>,
    day: Option<String>,
}
