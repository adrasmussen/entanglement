use std::sync::Arc;

use anyhow::Result;
use api::library::Library;

use common::{
    config::ESConfig,
    db::{DbBackend, MariaDBBackend, PostgresBackend},
};

pub async fn add(config: Arc<ESConfig>, path: String, uid: String, gid: String) -> Result<()> {
    let db: Box<dyn DbBackend> = match config.db_backend {
        common::config::DbBackend::MariaDB => Box::new(MariaDBBackend::new(config).await?),
        common::config::DbBackend::Postgres => Box::new(PostgresBackend::new(config).await?),
    };

    let library = Library {
        path,
        uid,
        gid,
        count: 0,
    };

    let uuid = db.add_library(library).await?;

    println!("created library {uuid}");

    Ok(())
}
