//! History entry model.

use serde::{Deserialize, Serialize};

/// A single clipboard transformation event stored in the database.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    /// Database primary key.
    pub id: i64,
    /// The raw clipboard text before pipeline processing.
    pub original_text: String,
    /// The clipboard text after all active rules have been applied.
    pub transformed_text: String,
    /// ISO-8601 timestamp of when the transformation occurred.
    pub created_at: String,
}
