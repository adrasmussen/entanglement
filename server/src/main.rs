use std::path::PathBuf;

use clap::Parser;
use tokio::signal::unix::{SignalKind, signal};
use tracing::info;
use tracing_subscriber::EnvFilter;

mod auth;
mod checks;
mod db;
mod debug;
mod fs;
mod http;
mod service;
mod task;

use api::{LINK_PATH, SLICE_PATH, THUMBNAIL_PATH};
use common::{config::read_config, db::MariaDBBackend};
use service::{ESMRegistry, EntanglementService};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "/etc/entanglement/config.toml")]
    config: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // console_subscriber::ConsoleLayer::builder()
    //     .retention(std::time::Duration::from_secs(1200))
    //     .server_addr(([0, 0, 0, 0], 6669))
    //     .init();

    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    info!("starting entanglement media management server");

    let config = read_config(PathBuf::from(args.config)).await;

    info!("performing filesystem sanity checks");

    checks::create_temp_file(&config.fs.media_srcdir).expect_err("media_srcdir is writeable");
    checks::create_temp_file(&config.fs.media_srvdir).expect("media_srvdir is not writeable");

    checks::subdir_exists(&config, LINK_PATH)
        .expect("could not create thumbnail path in media_srvdir");
    checks::subdir_exists(&config, THUMBNAIL_PATH)
        .expect("could not create thumbnail path in media_srvdir");
    checks::subdir_exists(&config, SLICE_PATH)
        .expect("could not create video slice path in media_srvdir");

    info!("starting core services");

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

    let mut sigint = signal(SignalKind::interrupt())?;

    tokio::select! {
        _ = sigint.recv() => {
            info!("caught SIGINT");
            Ok(())
        },
    }
}
