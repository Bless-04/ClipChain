/// Database-related constants.
pub mod db {
    /// Maximum number of clipboard history entries to retain.
    pub const MAX_HISTORY_ENTRIES: i64 = 500;

    /// SQLite database file name (placed in the Tauri app data directory).
    pub const DB_FILE_NAME: &str = "clipflow.db";
}
