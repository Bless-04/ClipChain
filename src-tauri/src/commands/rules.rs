//! Tauri command handlers for rule management.
//!
//! These are intentionally thin coordinators. Business logic belongs in
//! `pipeline/` and `db/`; command handlers only orchestrate and translate errors.
//!
use crate::{
    db,
    error::AppError,
    models::{events::RulesUpdated, Rule},
    pipeline,
    state::AppState,
};
use std::sync::Arc;
use tauri_specta::Event;

/// Returns the full list of saved rules in their pipeline order.
///
/// # Errors
///
/// Returns [`AppError::Database`] if the database query fails.
#[tauri::command]
#[specta::specta]
pub async fn get_rules(state: tauri::State<'_, Arc<AppState>>) -> Result<Vec<Rule>, AppError> {
    db::rules::get_all(&state.pool).await
}

/// Persists a new or updated rule.
///
/// If `rule` has `id == 0`, a new row is inserted. Otherwise the existing row
/// is updated in-place.
///
/// # Errors
///
/// Returns [`AppError::Database`] if the upsert fails, or
/// [`AppError::Validation`] if the rule configuration is invalid.
#[tauri::command]
#[specta::specta]
pub async fn save_rule(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    rule: Rule,
) -> Result<Rule, AppError> {
    let _guard = state.gate.lock().await;
    let saved = db::rules::save(&state.pool, rule).await?;
    // Persistence succeeded even if a window closed before event delivery.
    let _ = RulesUpdated.emit(&app);
    Ok(saved)
}

/// Permanently deletes the rule identified by `id`.
///
/// # Errors
///
/// Returns [`AppError::NotFound`] if no rule with that `id` exists.
#[tauri::command]
#[specta::specta]
pub async fn delete_rule(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    id: i64,
) -> Result<(), AppError> {
    let _guard = state.gate.lock().await;
    db::rules::delete(&state.pool, id).await?;
    let _ = RulesUpdated.emit(&app);
    Ok(())
}

/// Reorders the whole pipeline atomically.
/// # Errors
/// Rejects a stale or invalid permutation, or a database failure.
#[tauri::command]
#[specta::specta]
pub async fn reorder_rules(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    ids: Vec<i64>,
) -> Result<Vec<Rule>, AppError> {
    let _guard = state.gate.lock().await;
    let rules = db::rules::reorder(&state.pool, &ids).await?;
    let _ = RulesUpdated.emit(&app);
    Ok(rules)
}

/// Tests a draft rule using the same Rust engine as the clipboard monitor.
/// # Errors
/// Returns validation errors for invalid drafts, input, or excessive output.
#[tauri::command]
#[specta::specta]
pub async fn preview_rule(rule: Rule, input: String) -> Result<String, AppError> {
    pipeline::validation::validate_rule(&rule)?;
    pipeline::transformer::apply_rule(&rule, &input)
}
