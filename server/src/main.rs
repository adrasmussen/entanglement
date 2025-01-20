use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

mod auth;
mod db;
mod fs;
mod http;
mod service;

use common::config::ESConfig;
use service::{EntanglementService, ServiceType};
// the outermost caller should definitely have a loop that periodically calls
// Status for each service to ensure that the threads haven't stopped, and then
// gracefully stop the server after logging whatever the error was

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    // temporary dummy configuration
    let config = Arc::new(ESConfig {
        group_yaml: Some(String::from("/srv/home/alex/workspace/entanglement/dev/groups.yml")),
        http_socket: String::from("[::]:8080"),
        http_url_root: String::from("/entanglement"),
        http_doc_root: String::from("/srv/home/alex/workspace/entanglement/webapp/dist"),
        mysql_url: String::from("mysql://entanglement:testpw@[fd00::3]/entanglement"),
        media_srcdir: PathBuf::from("/srv/home/alex/workspace/entanglement/dev/src"),
        media_linkdir: PathBuf::from("/srv/home/alex/workspace/entanglement/dev/srv"),
    });

    let (db_sender, db_svc) = db::mysql::MySQLService::create(config.clone());
    let (fs_sender, fs_svc) = fs::svc::FileService::create(config.clone());
    let (auth_sender, auth_svc) = auth::svc::AuthService::create(config.clone());
    let (http_sender, http_svc) = http::svc::HttpService::create(config.clone());

    let mut senders = HashMap::new();

    senders.insert(ServiceType::Db, db_sender);
    senders.insert(ServiceType::Fs, fs_sender);
    senders.insert(ServiceType::Auth, auth_sender);
    senders.insert(ServiceType::Http, http_sender);

    db_svc.start(senders.clone()).await?;
    fs_svc.start(senders.clone()).await?;
    auth_svc.start(senders.clone()).await?;
    http_svc.start(senders.clone()).await?;

    loop {}
}
