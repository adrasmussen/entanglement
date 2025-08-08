use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand, arg, command};

use common::config::read_config;

mod svc;
use svc::serve_http;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// config file
    #[arg(short, long, default_value = "/etc/entanglement/config.toml")]
    config: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// test the connection from a reverse proxy
    ConnCheck {
        #[command(subcommand)]
        mode: ConnMode,
    },
}

#[derive(Clone, PartialEq, Subcommand)]
enum ConnMode {
    /// check if the reverse proxy can connect
    #[command()]
    Proxy,

    /// check that a user can connect with their own cert
    #[command()]
    User,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let config = read_config(PathBuf::from(cli.config)).await;

    if let Some(cmd) = &cli.command {
        match cmd {
            Commands::ConnCheck { mode } => {
                serve_http(config.clone(), mode).await;
            }
        }
    }

    Ok(())
}
