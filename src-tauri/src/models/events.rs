//! Typed native events and monitor diagnostics, generated into TypeScript.

use super::HistoryEntry;
use serde::{Deserialize, Serialize};

/// Current availability of the native clipboard worker.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MonitorStatus {
    /// True once the worker has successfully polled while enabled.
    pub is_running: bool,
    /// Most recent operational error; clipboard text is never included.
    pub last_error: Option<String>,
}

/// Emitted after a successful transformation; text is omitted when history is disabled.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardChanged {
    /// Persisted entry, if history recording is enabled and insertion succeeded.
    pub entry: Option<HistoryEntry>,
}

/// Signals that the frontend must reload its rule view model.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct RulesUpdated;

/// Signals that settings changed.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct SettingsUpdated;

/// Signals that retained history was cleared.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct HistoryCleared;

/// Signals a change in clipboard availability or its last operational error.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct MonitorChanged(pub MonitorStatus);
