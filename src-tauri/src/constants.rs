/// Database-related constants.
pub mod db {
    /// Maximum number of clipboard history entries to retain.
    pub const MAX_HISTORY_ENTRIES: i64 = 500;

    /// `SQLite` file name in the `ClipChain` application data directory.
    pub const DB_FILE_NAME: &str = "clipchain.db";
pub mod app {
    /// Label of the application's main window.
    pub const MAIN_WINDOW: &str = "main";
}

/// Tauri event name constants.
pub mod events {
    /// Emitted whenever the clipboard content changes after pipeline processing.
    pub const CLIPBOARD_CHANGED: &str = "clipboard-changed";

    /// Emitted whenever the rule set is mutated (add / delete / reorder).
    pub const RULES_UPDATED: &str = "rules-updated";
}
