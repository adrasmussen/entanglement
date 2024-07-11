use std::collections::HashSet;

#[derive(Debug)]
pub struct User {
    pub uid: String,
    pub library: String,
    pub settings: UserSettings
}

#[derive(Debug)]
pub struct UserSettings {
    pub theme: Option<String>,
}

#[derive(Debug)]
pub struct Group {
    pub gid: String,
    pub members: HashSet<String>
}
