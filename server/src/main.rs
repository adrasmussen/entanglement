use std::path::PathBuf;

use tracing::{info, Level};
use tracing_subscriber::{
    filter::FilterFn,
    layer::{Layer, SubscriberExt},
    util::SubscriberInitExt,
};

mod auth;
mod checks;
mod db;
mod fs;
mod http;
mod service;
mod task;

use api::{ORIGINAL_PATH, SLICE_PATH, THUMBNAIL_PATH};
use common::{config::read_config, db::MariaDBBackend};
use service::{ESMRegistry, EntanglementService};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let crate_filter = FilterFn::new(|metadata| !metadata.target().starts_with("h2"))
        .with_max_level_hint(Level::INFO);

    let fmt_layer = tracing_subscriber::fmt::layer();

    tracing_subscriber::registry()
        .with(fmt_layer.with_filter(crate_filter))
        .init();

    info!("entanglement server starting up, processing config file");

    let config = read_config(PathBuf::from(
        "/srv/home/alex/workspace/entanglement/dev/config.toml",
    ))
    .await;

    info!("performing filesystem sanity checks");

    // sanity checks
    checks::create_temp_file(&config.fs.media_srcdir).expect_err("media_srcdir is writeable");
    checks::create_temp_file(&config.fs.media_srvdir).expect("media_srvdir is not writeable");

    checks::subdir_exists(&config, ORIGINAL_PATH)
        .expect("could not create thumbnail path in media_srvdir");
    checks::subdir_exists(&config, THUMBNAIL_PATH)
        .expect("could not create thumbnail path in media_srvdir");
    checks::subdir_exists(&config, SLICE_PATH)
        .expect("could not create video slice path in media_srvdir");

    info!("starting core services");

    // start the core services
    let registry = ESMRegistry::new();

    let auth_svc = auth::svc::AuthService::create(config.clone(), &registry);
    let db_svc = db::svc::DbService::<MariaDBBackend>::create(config.clone(), &registry);
    let http_svc = http::svc::HttpService::create(config.clone(), &registry);
    let task_svc = task::svc::TaskService::create(config.clone(), &registry);

    auth_svc.start(&registry).await?;
    db_svc.start(&registry).await?;
    http_svc.start(&registry).await?;
    task_svc.start(&registry).await?;

    info!("startup complete!");

    loop {}
}
