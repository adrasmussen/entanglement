use std::collections::{HashMap, HashSet};
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
        auth_admin_groups: HashSet::from([String::from("admin")]),
        auth_yaml_groups: Some(String::from(
            "/srv/home/alex/workspace/entanglement/dev/groups.yml",
        )),
        http_socket: String::from("[::]:8080"),
        http_url_root: String::from("/entanglement"),
        http_doc_root: String::from("/srv/home/alex/workspace/entanglement/webapp/dist"),
        mysql_url: String::from("mysql://entanglement:testpw@[fd00::3]/entanglement"),
        media_srcdir: PathBuf::from("/srv/home/alex/workspace/entanglement/dev/src"),
        media_linkdir: PathBuf::from("/srv/home/alex/workspace/entanglement/dev/srv"),
    });

    let mut senders = HashMap::new();

    let auth_svc = auth::svc::AuthService::create(config.clone(), &mut senders);
    let db_svc = db::mysql::MySQLService::create(config.clone(), &mut senders);
    let fs_svc = fs::svc::FileService::create(config.clone(), &mut senders);
    let http_svc = http::svc::HttpService::create(config.clone(), &mut senders);

    auth_svc.start(&senders).await?;
    db_svc.start(&senders).await?;
    fs_svc.start(&senders).await?;
    http_svc.start(&senders).await?;

    loop {}
}
