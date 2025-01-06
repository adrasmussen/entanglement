use std::collections::HashSet;

pub struct User {
    pub uid: String,
    pub name: String,
    pub groups: HashSet<String>,
}

pub struct Group {
    pub gid: String,
    pub name: String,
    pub members: HashSet<String>,
}
