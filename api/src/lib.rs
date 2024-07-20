pub mod image;
pub mod auth;
pub mod ticket;

pub type MediaUuid = u64;

pub struct PartialDate {
    year: Option<String>,
    month: Option<String>,
    day: Option<String>,
}
