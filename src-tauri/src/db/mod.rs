//! `SQLite` persistence. SQL and migrations are confined to this module.

pub mod history;
pub mod rules;
pub mod settings;

use crate::{constants::db, error::AppError};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    SqlitePool,
};
use std::{path::Path, time::Duration};

/// Opens the local database and applies embedded, numbered migrations.
///
/// # Errors
/// Returns a database error if opening the file or migrating it fails.
pub async fn open(path: &Path) -> Result<SqlitePool, AppError> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_millis(db::BUSY_TIMEOUT_MS));
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await?;
    sqlx::migrate!("./src/db/migrations").run(&pool).await?;
    Ok(pool)
}

#[cfg(test)]
mod tests;
