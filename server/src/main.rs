use std::path::PathBuf;

use tracing::info;

mod auth;
mod checks;
mod db;
mod fs;
mod http;
mod service;
mod task;

use api::{ORIGINAL_PATH, SLICE_PATH, THUMBNAIL_PATH};
use common::config::read_config;
use service::{ESMRegistry, EntanglementService};

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
    let config = read_config(PathBuf::from(
        "/srv/home/alex/workspace/entanglement/dev/config.toml",
    ))
    .await;

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
    let registry = ESMRegistry::new();

    let auth_svc = auth::svc::AuthService::create(config.clone(), &registry);
    let db_svc = db::mariadb::MariaDBService::create(config.clone(), &registry);
    let fs_svc = fs::svc::FileService::create(config.clone(), &registry);
    let http_svc = http::svc::HttpService::create(config.clone(), &registry);

    auth_svc.start(&registry).await?;
    db_svc.start(&registry).await?;
    fs_svc.start(&registry).await?;
    http_svc.start(&registry).await?;

    info!("done");

    info!("startup complete!");

    loop {}
}
