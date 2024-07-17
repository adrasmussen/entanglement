use std::collections::HashSet;

use gloo_net::http::Request;

use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub uid: String,
    pub groups: HashSet<String>,
    pub library: String,
    pub settings: UserSettings
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserSettings {
    pub theme: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Group {
    pub gid: String,
    pub members: HashSet<String>
}

// add_user

#[derive(Serialize, Deserialize)]
pub struct AddUserReq {
    user: User,
}

#[derive(Serialize, Deserialize)]
pub struct AddUserResp {
    resp: String,
}

async fn add_user(req: AddUserReq) -> anyhow::Result<AddUserResp> {
    let resp: AddUserResp = Request::post("/api/user/add").json(&req)?.send().await?.json().await?;
    Ok(resp)
}

//
