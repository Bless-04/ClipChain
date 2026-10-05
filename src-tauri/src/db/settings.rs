//! Singleton settings repository.

use crate::{error::AppError, models::AppSettings};
use sqlx::SqlitePool;

/// Returns stored settings or documented defaults for a fresh database.
/// # Errors
/// Returns database or stored JSON errors.
pub async fn get(pool: &SqlitePool) -> Result<AppSettings, AppError> {
    let payload: Option<String> = sqlx::query_scalar("SELECT payload FROM settings WHERE id = 1")
        .fetch_optional(pool)
        .await?;
    payload.map_or_else(
        || Ok(AppSettings::default()),
        |payload| Ok(serde_json::from_str(&payload)?),
    )
}

/// Persists the complete settings object in one statement.
/// # Errors
/// Returns a database error if serialization or persistence fails.
pub async fn save(pool: &SqlitePool, settings: &AppSettings) -> Result<(), AppError> {
    sqlx::query("INSERT INTO settings (id, payload) VALUES (1, ?) ON CONFLICT(id) DO UPDATE SET payload = excluded.payload")
        .bind(serde_json::to_string(settings)?).execute(pool).await?;
    Ok(())
}
