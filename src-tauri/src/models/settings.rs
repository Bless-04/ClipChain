//! Application settings model.

use serde::{Deserialize, Serialize};

/// Global `ClipChain` settings persisted in `SQLite`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// When `false`, the clipboard monitor is paused entirely.
    pub is_active: bool,
    /// Whether `ClipChain` should register itself in the OS login items.
    pub launch_at_startup: bool,
    /// Save before/after text locally; disabled history does not affect transformation.
    #[serde(default = "default_record_history")]
    pub record_history: bool,
}

const fn default_record_history() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            is_active: true,
            launch_at_startup: false,
            record_history: true,
        }
    }
}
