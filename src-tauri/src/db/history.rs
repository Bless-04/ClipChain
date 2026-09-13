//! Bounded clipboard history repository.

use crate::{constants::db, error::AppError, models::HistoryEntry};
use sqlx::SqlitePool;

/// Lists the most recent retained transformations, newest first.
/// # Errors
/// Returns a database error on query failure.
pub async fn get_all(pool: &SqlitePool) -> Result<Vec<HistoryEntry>, AppError> {
    let rows: Vec<(i64, String, String, String)> = sqlx::query_as(
        "SELECT id, original_text, transformed_text, created_at FROM history ORDER BY id DESC LIMIT ?"
    ).bind(db::MAX_HISTORY_ENTRIES).fetch_all(pool).await?;
    Ok(rows
        .into_iter()
        .map(
            |(id, original_text, transformed_text, created_at)| HistoryEntry {
                id,
                original_text,
                transformed_text,
                created_at,
            },
        )
        .collect())
}

/// Records a completed clipboard write and prunes excess history atomically.
/// # Errors
/// Returns a database error if insertion or retention fails.
pub async fn insert(
    pool: &SqlitePool,
    original: &str,
    transformed: &str,
) -> Result<HistoryEntry, AppError> {
    let mut transaction = pool.begin().await?;
    let (id, created_at): (i64, String) = sqlx::query_as(
        "INSERT INTO history (original_text, transformed_text) VALUES (?, ?) RETURNING id, created_at"
    ).bind(original).bind(transformed).fetch_one(&mut *transaction).await?;
    sqlx::query(
        "DELETE FROM history WHERE id NOT IN (SELECT id FROM history ORDER BY id DESC LIMIT ?)",
    )
    .bind(db::MAX_HISTORY_ENTRIES)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(HistoryEntry {
        id,
        original_text: original.into(),
        transformed_text: transformed.into(),
        created_at,
    })
}

/// Clears all history. This does not change rules or settings.
/// # Errors
/// Returns a database error if deletion fails.
pub async fn clear(pool: &SqlitePool) -> Result<(), AppError> {
    sqlx::query("DELETE FROM history").execute(pool).await?;
    Ok(())
}
