use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, arg, command};

use common::config::read_config;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// config file
    #[arg(short, long, default_value = "/etc/entanglement/config.toml")]
    config: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let _config = read_config(PathBuf::from(cli.config)).await;

    Ok(())
}
