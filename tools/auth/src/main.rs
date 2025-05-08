use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum, arg, command};

use common::{
    auth::{
        AuthnBackend, AuthzBackend,
        ldap::LdapAuthz,
        tomlfile::{TomlAuthnFile, TomlAuthzFile},
    },
    config::read_config,
};

#[derive(Clone, Debug, ValueEnum)]
enum CliAuthn {
    TomlFile,
}

#[derive(Clone, Debug, ValueEnum)]
enum CliAuthz {
    Ldap,
    TomlFile,
}

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
    /// authorization operations
    Authz {
        /// backend to query
        #[arg(short, long)]
        backend: CliAuthz,

        #[command(subcommand)]
        authzcmd: AuthzCommands,
    },

    /// authentication operations
    Authn {
        /// backend to query
        #[arg(short, long)]
        backend: CliAuthn,

        #[command(subcommand)]
        authncmd: AuthnCommands,
    },
}

#[derive(Subcommand)]
enum AuthzCommands {
    /// show users in a group
    #[command(name = "members")]
    UsersInGroup {
        /// group to inspect
        #[arg()]
        gid: String,
    },

    /// show groups containing a user
    #[command(name = "groups")]
    GroupsForUser {
        /// user to inspect
        #[arg()]
        uid: String,
    },
}

#[derive(Subcommand)]
enum AuthnCommands {
    /// authenticate a user
    #[command(name = "auth")]
    AuthenticateUser {
        /// user
        #[arg()]
        uid: String,

        /// password
        #[arg(short, long)]
        password: String,
    },

    /// check if a user is valid
    #[command(name = "validate")]
    IsValidUser {
        /// user
        #[arg()]
        uid: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let config = read_config(PathBuf::from(cli.config)).await;

    if let Some(cmd) = &cli.command {
        match cmd {
            Commands::Authz { backend, authzcmd } => {
                let backend: Box<dyn AuthzBackend> = match backend {
                    CliAuthz::Ldap => Box::new(LdapAuthz::new(config.clone())?),
                    CliAuthz::TomlFile => Box::new(TomlAuthzFile::new(config.clone())?),
                };

                match authzcmd {
                    AuthzCommands::UsersInGroup { gid } => {
                        let users = backend.users_in_group(gid.clone()).await?;

                        println!("users in group: {users:#?}");
                    }
                    AuthzCommands::GroupsForUser { uid } => {
                        let groups = backend.users_in_group(uid.clone()).await?;

                        println!("groups containing user: {groups:#?}");
                    }
                }
            }
            Commands::Authn { backend, authncmd } => {
                let backend: Box<dyn AuthnBackend> = match backend {
                    CliAuthn::TomlFile => Box::new(TomlAuthnFile::new(config.clone())?),
                };

                match authncmd {
                    AuthnCommands::AuthenticateUser { uid, password } => {
                        if backend
                            .authenticate_user(uid.clone(), password.clone())
                            .await?
                        {
                            println!("authentication successful")
                        } else {
                            println!("authentication failed")
                        }
                    }
                    AuthnCommands::IsValidUser { uid } => {
                        if backend.is_valid_user(uid.clone()).await? {
                            println!("user is valid")
                        } else {
                            println!("user is invalid")
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
