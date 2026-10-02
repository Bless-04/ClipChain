//! Tauri command handlers for application settings.

use crate::{
    db,
    error::AppError,
    models::{
        events::{MonitorStatus, SettingsUpdated},
        AppSettings,
    },
    state::AppState,
};
use std::sync::Arc;
use tauri_plugin_autostart::ManagerExt;
use tauri_specta::Event;

/// Returns the current application settings.
///
/// Falls back to [`AppSettings::default`] if no settings row exists yet.
///
/// # Errors
///
/// Returns [`AppError::Database`] if the database query fails.
#[tauri::command]
#[specta::specta]
pub async fn get_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<AppSettings, AppError> {
    let _guard = state.gate.lock().await;
    let mut settings = db::settings::get(&state.pool).await?;
    settings.launch_at_startup = app
        .autolaunch()
        .is_enabled()
        .map_err(|error| AppError::Internal(error.to_string()))?;
    Ok(settings)
}

/// Persists updated application settings.
///
/// # Errors
///
/// Returns [`AppError::Database`] if the upsert fails.
#[tauri::command]
#[specta::specta]
pub async fn save_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    settings: AppSettings,
) -> Result<AppSettings, AppError> {
    let _guard = state.gate.lock().await;
    let manager = app.autolaunch();
    let previous = manager
        .is_enabled()
        .map_err(|error| AppError::Internal(error.to_string()))?;
    persist_with_startup(&state, &settings, previous, |enabled| {
        if enabled {
            manager.enable()
        } else {
            manager.disable()
        }
        .map_err(|error| error.to_string())
    })
    .await?;
    let _ = SettingsUpdated.emit(&app);
    Ok(settings)
}

// Keep the OS operation injectable so compensation can be tested without
// changing the user's startup registration. The command holds the mutation gate.
async fn persist_with_startup(
    state: &AppState,
    settings: &AppSettings,
    previous: bool,
    set_startup: impl Fn(bool) -> Result<(), String>,
) -> Result<(), AppError> {
    if previous != settings.launch_at_startup {
        set_startup(settings.launch_at_startup).map_err(|error| {
            AppError::Internal(format!("Could not change startup registration: {error}"))
        })?;
    }
    if let Err(error) = state.persist_settings(settings).await {
        if previous != settings.launch_at_startup {
            if let Err(rollback_error) = set_startup(previous) {
                return Err(AppError::Internal(format!(
                    "{error}; startup rollback also failed: {rollback_error}"
                )));
            }
        }
        return Err(error);
    }
    Ok(())
}

/// Returns the native worker's current availability and last operational error.
/// # Errors
/// This command currently always succeeds; the result preserves the IPC error contract.
#[tauri::command]
#[specta::specta]
pub async fn get_monitor_status(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<MonitorStatus, AppError> {
    Ok(state.monitor.read().await.clone())
}

#[cfg(test)]
mod tests;
