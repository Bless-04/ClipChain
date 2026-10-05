//! Domain models — public re-exports.
//!
//! All types that cross the IPC boundary are declared here and derive
//! `specta::Type` so that `tauri-specta` can generate TypeScript bindings.

pub mod events;
pub mod history;
pub mod rule;
pub mod settings;

// Flatten the most-used types to `models::*` for ergonomic imports.
pub use history::HistoryEntry;
pub use rule::{RegexRule, ReplaceRule, Rule, RuleType, TrimRule};
pub use settings::AppSettings;
