//! Tauri command handlers for clipboard transformation history.

use crate::{
    db,
    error::AppError,
    models::{events::HistoryCleared, HistoryEntry},
    state::AppState,
};
use std::sync::Arc;
use tauri_specta::Event;

/// Returns the N most recent clipboard transformation history entries.
///
/// Results are ordered by `created_at` descending (newest first).
///
/// # Errors
///
/// Returns [`AppError::Database`] if the database query fails.
#[tauri::command]
#[specta::specta]
pub async fn get_history(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Vec<HistoryEntry>, AppError> {
    db::history::get_all(&state.pool).await
}

/// Deletes all history entries from the database.
///
/// # Errors
///
/// Returns [`AppError::Database`] if the delete fails.
#[tauri::command]
#[specta::specta]
pub async fn clear_history(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    let _guard = state.gate.lock().await;
    db::history::clear(&state.pool).await?;
    let _ = HistoryCleared.emit(&app);
    Ok(())
}
