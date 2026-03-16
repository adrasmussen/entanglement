use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use common::config::read_config;

mod dump;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// config file
    #[arg(short, long, default_value = "/etc/entanglement/config.toml")]
    config: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// dump the db to a rocksdb directory
    Dump {
        #[arg(short, long)]
        directory: PathBuf,
    },
    /// restore the db from a rocksdb directory
    Undump {
        #[arg(short, long)]
        directory: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let config = read_config(PathBuf::from(cli.config)).await;

    match cli.command {
        Command::Dump { directory } => dump::dump(config, directory).await?,
        Command::Undump { directory } => dump::undump(config, directory).await?,
    }

    Ok(())
}
