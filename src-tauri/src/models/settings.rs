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

#[cfg(test)]
mod tests {
    use super::AppSettings;

    #[test]
    fn legacy_settings_default_recording_on_and_preserve_explicit_false(
    ) -> Result<(), serde_json::Error> {
        let legacy: AppSettings =
            serde_json::from_str(r#"{"isActive":false,"launchAtStartup":true}"#)?;
        assert!(legacy.record_history);
        assert!(!legacy.is_active);
        assert!(legacy.launch_at_startup);
        let disabled = AppSettings {
            record_history: false,
            ..legacy
        };
        let json = serde_json::to_value(&disabled)?;
        assert_eq!(json["recordHistory"], false);
        assert_eq!(serde_json::from_value::<AppSettings>(json)?, disabled);
        Ok(())
    }
}
