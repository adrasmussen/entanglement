use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use tracing::info;

mod auth;
mod checks;
mod db;
mod fs;
mod http;
mod service;

use api::{ORIGINAL_PATH, SLICE_PATH, THUMBNAIL_PATH};
use common::config::ESConfig;
use service::EntanglementService;

// the outermost caller should definitely have a loop that periodically calls
// Status for each service to ensure that the threads haven't stopped, and then
// gracefully stop the server after logging whatever the error was
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    info!("entanglement server starting up, processing config file...");

    // temporary dummy configuration -- this will eventually come from a parser
    let config = Arc::new(ESConfig {
        authn_proxy_header: Some(String::from("proxy-user")),
        authz_admin_groups: HashSet::from([String::from("admin")]),
        authz_yaml_groups: Some(String::from(
            "/srv/home/alex/workspace/entanglement/dev/groups.yml",
        )),
        http_socket: String::from("[::]:8080"),
        http_url_root: String::from("/entanglement"),
        http_doc_root: String::from(
            "/srv/home/alex/workspace/entanglement/target/dx/webapp/debug/web/public",
        ),
        mariadb_url: String::from("mysql://entanglement:testpw@[fd00::3]/entanglement"),
        media_srcdir: PathBuf::from("/srv/home/alex/workspace/entanglement/dev/src"),
        media_srvdir: PathBuf::from("/srv/home/alex/workspace/entanglement/dev/srv"),
        fs_scanner_threads: 8,
    });

    info!("done");

    info!("performing filesystem sanity checks...");

    // sanity checks
    checks::create_temp_file(&config.media_srcdir).expect_err("media_srcdir is writeable");
    checks::create_temp_file(&config.media_srvdir).expect("media_srvdir is not writeable");

    checks::subdir_exists(&config, ORIGINAL_PATH)
        .expect("could not create thumbnail path in media_srvdir");
    checks::subdir_exists(&config, THUMBNAIL_PATH)
        .expect("could not create thumbnail path in media_srvdir");
    checks::subdir_exists(&config, SLICE_PATH)
        .expect("could not create video slice path in media_srvdir");

    info!("done");

    info!("starting core services...");

    // start the core services
    let mut senders = HashMap::new();

    let auth_svc = auth::svc::AuthService::create(config.clone(), &mut senders);
    let db_svc = db::mariadb::MariaDBService::create(config.clone(), &mut senders);
    let fs_svc = fs::svc::FileService::create(config.clone(), &mut senders);
    let http_svc = http::svc::HttpService::create(config.clone(), &mut senders);

    auth_svc.start(&senders).await?;
    db_svc.start(&senders).await?;
    fs_svc.start(&senders).await?;
    http_svc.start(&senders).await?;

    info!("done");

    info!("startup complete!");

    loop {}
}
