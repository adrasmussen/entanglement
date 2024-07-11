use std::string;

pub mod image;
pub mod auth;

pub struct PartialDate {
    year: Option<String>,
    month: Option<String>,
    day: Option<String>,
}
