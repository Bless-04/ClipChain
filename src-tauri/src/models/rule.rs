//! Domain models for `ClipChain` rules.
//!
//! Every type in this module derives `Serialize`, `Deserialize`, and `specta::Type`
//! so that `tauri-specta` can generate the corresponding TypeScript bindings.
//! **Never handwrite TypeScript types that correspond to these structs.**

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// RuleType — discriminant enum
// ---------------------------------------------------------------------------

/// Identifies the variant of a transformation rule.
///
/// This is the **single source of truth** for rule identity per AGENTS.md §6.
/// It is generated into TypeScript as a string-literal union — never duplicate it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum RuleType {
    /// Removes leading/trailing whitespace.
    Trim,
    /// Replaces a literal target string with a replacement.
    Replace,
    /// Applies a compiled regex substitution.
    Regex,
}

// ---------------------------------------------------------------------------
// TrimRule
// ---------------------------------------------------------------------------

/// Configuration for a whitespace-trimming rule.
///
/// The user can independently enable trimming from the start and/or end of
/// the clipboard text. Both flags default to `true` for a "full trim" experience.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TrimRule {
    /// Database primary key.
    pub id: i64,
    /// Human-readable display name shown in the rules list.
    pub name: String,
    /// Execution order in the pipeline (ascending, 0-indexed).
    pub order: i32,
    /// Whether this rule participates in pipeline execution.
    pub is_active: bool,
    /// Remove leading whitespace when `true`.
    pub trim_start: bool,
    /// Remove trailing whitespace when `true`.
    pub trim_end: bool,
}

// ---------------------------------------------------------------------------
// ReplaceRule
// ---------------------------------------------------------------------------

/// Configuration for a literal string-replacement rule.
///
/// Case-insensitive matching is handled in pure Rust via `.to_lowercase()`
/// comparisons — the `regex` crate is intentionally NOT used here to preserve
/// bare-metal performance on the clipboard monitor hot path.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceRule {
    /// Database primary key.
    pub id: i64,
    /// Human-readable display name shown in the rules list.
    pub name: String,
    /// Execution order in the pipeline (ascending, 0-indexed).
    pub order: i32,
    /// Whether this rule participates in pipeline execution.
    pub is_active: bool,
    /// The literal string to search for.
    pub target: String,
    /// The string to substitute in place of every match.
    pub replacement: String,
    /// When `true`, the search is case-sensitive. When `false`, case is ignored.
    pub match_case: bool,
}

// ---------------------------------------------------------------------------
// RegexRule
// ---------------------------------------------------------------------------

/// Configuration for a regex-based substitution rule.
///
/// The `pattern` is a standard Rust `regex` crate pattern.
/// On the frontend, a visual fluent builder constructs `pattern` so users
/// never need to type raw regex syntax manually.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RegexRule {
    /// Database primary key.
    pub id: i64,
    /// Human-readable display name shown in the rules list.
    pub name: String,
    /// Execution order in the pipeline (ascending, 0-indexed).
    pub order: i32,
    /// Whether this rule participates in pipeline execution.
    pub is_active: bool,
    /// A valid regex pattern string (compiled by the `regex` crate at runtime).
    pub pattern: String,
    /// Replacement string. Supports regex capture-group references (e.g. `$1`).
    pub replacement: String,
}

// ---------------------------------------------------------------------------
// Rule — top-level discriminated union
// ---------------------------------------------------------------------------

/// The top-level rule enum crossing the IPC boundary.
///
/// Uses `#[serde(tag = "type", content = "data")]` so TypeScript receives:
/// ```json
/// { "type": "trim", "data": { "id": 1, "name": "...", ... } }
/// ```
/// This maps cleanly to a TypeScript discriminated union in the generated bindings.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum Rule {
    /// Whitespace trimming rule variant.
    Trim(TrimRule),
    /// String replacement rule variant.
    Replace(ReplaceRule),
    /// Regular expression substitution rule variant.
    Regex(RegexRule),
}

impl Rule {
    /// Returns the persisted identifier; zero means a new rule.
    #[must_use]
    pub fn id(&self) -> i64 {
        match self {
            Self::Trim(rule) => rule.id,
            Self::Replace(rule) => rule.id,
            Self::Regex(rule) => rule.id,
        }
    }

    /// Returns the human-readable name.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Trim(rule) => &rule.name,
            Self::Replace(rule) => &rule.name,
            Self::Regex(rule) => &rule.name,
        }
    }

    /// Sets database-owned identity and pipeline position.
    pub fn set_identity(&mut self, id: i64, order: i32) {
        match self {
            Self::Trim(rule) => {
                rule.id = id;
                rule.order = order;
            }
            Self::Replace(rule) => {
                rule.id = id;
                rule.order = order;
            }
            Self::Regex(rule) => {
                rule.id = id;
                rule.order = order;
            }
        }
    }

    /// Returns the execution order for pipeline sorting.
    #[must_use]
    pub fn order(&self) -> i32 {
        match self {
            Rule::Trim(r) => r.order,
            Rule::Replace(r) => r.order,
            Rule::Regex(r) => r.order,
        }
    }

    /// Returns whether this rule is active and should be executed.
    #[must_use]
    pub fn is_active(&self) -> bool {
        match self {
            Rule::Trim(r) => r.is_active,
            Rule::Replace(r) => r.is_active,
            Rule::Regex(r) => r.is_active,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_enum_serializes_with_tag_and_content() {
        let rule = Rule::Trim(TrimRule {
            id: 1,
            name: "Strip whitespace".to_string(),
            order: 0,
            is_active: true,
            trim_start: true,
            trim_end: true,
        });
        let json = serde_json::to_value(&rule).expect("serialization must succeed in tests");
        assert_eq!(json["type"], "trim");
        assert!(json["data"].is_object());
        assert_eq!(json["data"]["trimStart"], true);
    }

    #[test]
    fn replace_rule_serializes_camel_case() {
        let rule = Rule::Replace(ReplaceRule {
            id: 2,
            name: "Redact API key".to_string(),
            order: 1,
            is_active: true,
            target: "secret".to_string(),
            replacement: "[REDACTED]".to_string(),
            match_case: false,
        });
        let json = serde_json::to_value(&rule).expect("serialization must succeed in tests");
        assert_eq!(json["type"], "replace");
        assert_eq!(json["data"]["matchCase"], false);
    }

    #[test]
    fn rule_order_and_is_active_accessors_work() {
        let rule = Rule::Regex(RegexRule {
            id: 3,
            name: "Phone redactor".to_string(),
            order: 5,
            is_active: false,
            pattern: r"\d{10}".to_string(),
            replacement: "[PHONE]".to_string(),
        });
        assert_eq!(rule.order(), 5);
        assert!(!rule.is_active());
    }
}
