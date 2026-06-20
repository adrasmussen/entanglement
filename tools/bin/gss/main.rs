use std::{path::PathBuf, time::Instant};

use anyhow::Result;
use clap::Parser;

use common::config::read_config;
use libgssapi::{
    context::{ClientCtx, CtxFlags, ServerCtx},
    credential::{Cred, CredUsage},
    name::Name,
    oid::{GSS_MECH_KRB5, GSS_NT_HOSTBASED_SERVICE, OidSet},
};

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

    let sname = Name::new(
        "entg@bifrost.nonlocal.cloud".to_string().as_bytes(),
        Some(GSS_NT_HOSTBASED_SERVICE),
    )?;

    let mechs = OidSet::singleton(GSS_MECH_KRB5)?;

    let now = Instant::now();

    let server_cred = Cred::acquire(None, None, CredUsage::Accept, Some(&mechs))?;

    let mut server_ctx = ServerCtx::new(Some(server_cred));

    println!("server setup: {:#?}", Instant::now() - now);

    let now = Instant::now();

    let client_cred = Cred::acquire(None, None, CredUsage::Initiate, Some(&mechs))?;

    let mut client_ctx = ClientCtx::new(
        Some(client_cred),
        sname,
        CtxFlags::empty(),
        Some(GSS_MECH_KRB5),
    );

    println!("client setup: {:#?}", Instant::now() - now);

    let now = Instant::now();

    let token = client_ctx
        .step(None, None)?
        .ok_or_else(|| anyhow::Error::msg("seriously libgssapi"))?;

    println!("client step: {:#?}", Instant::now() - now);

    let now = Instant::now();

    server_ctx.step(&token, None)?;

    println!("server step: {:#?}", Instant::now() - now);

    Ok(())
}
