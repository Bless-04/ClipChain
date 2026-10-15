//! Centralized Rust-side constants for `ClipChain`.
//!
//! All string literals that carry semantic meaning are defined here.
//! Command handlers and service code must reference these constants —
//! never inline raw string literals.

/// Database-related constants.
pub mod db {
    /// Maximum number of clipboard history entries to retain.
    pub const MAX_HISTORY_ENTRIES: i64 = 500;

    /// `SQLite` file name in the `ClipChain` application data directory.
    pub const DB_FILE_NAME: &str = "clipchain.db";

    /// Bound waiting for another `SQLite` writer.
    pub const BUSY_TIMEOUT_MS: u64 = 5_000;
}

/// OS clipboard monitor constants.
pub mod clipboard {
    /// How often the clipboard monitor polls for new content, in milliseconds.
    pub const POLL_INTERVAL_MS: u64 = 250;

    /// Bound work and history size for a single clipboard value (UTF-8 bytes).
    pub const MAX_TEXT_BYTES: usize = 1_048_576;

    /// Worker thread name used by diagnostics.
    pub const WORKER_NAME: &str = "clipchain-clipboard";
}

/// Validation limits shared with the frontend through generated constants.
pub mod validation {
    /// Maximum persisted pipeline length.
    pub const MAX_RULES: usize = 100;
    /// Maximum rule name length in Unicode characters.
    pub const MAX_NAME_CHARS: usize = 120;
    /// Maximum pattern, search, or replacement size in UTF-8 bytes.
    pub const MAX_FIELD_BYTES: usize = 16_384;
    /// Maximum integer that JavaScript represents exactly.
    pub const MAX_SAFE_ID: i64 = 9_007_199_254_740_991;
}

/// Application identity needed by native window callbacks.
pub mod app {
    /// Application data identity shared by the desktop and CLI.
    pub const IDENTIFIER: &str = "com.blessing.clipchain";
    /// Label of the application's main window.
    pub const MAIN_WINDOW: &str = "main";
}

/// Tauri event name constants.
///
/// These strings are mirrored verbatim in `src/lib/constants/events.ts`.
/// Any change here requires a matching change on the TypeScript side.
pub mod events {
    /// Emitted whenever the clipboard content changes after pipeline processing.
    pub const CLIPBOARD_CHANGED: &str = "clipboard-changed";

    /// Emitted whenever the rule set is mutated (add / delete / reorder).
    pub const RULES_UPDATED: &str = "rules-updated";
}
