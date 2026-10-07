//! Unified application error type.
//!
//! `AppError` is the single error type returned by every fallible Tauri command.
//! Deriving `specta::Type` ensures TypeScript receives typed error payloads
//! rather than opaque strings, making frontend error-handling exhaustive.

use serde::{Deserialize, Serialize};

/// All errors that can surface from a Tauri command handler.
///
/// The `#[serde(tag = "kind", content = "message")]` representation produces
/// JSON like `{ "kind": "notFound", "message": "Rule 42 not found" }`,
/// which the generated TypeScript bindings expose as a discriminated union.
#[derive(Debug, Clone, thiserror::Error, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub enum AppError {
    /// Wraps `SQLx` / database layer errors.
    #[error("Database error: {0}")]
    Database(String),

    /// Returned when a requested resource does not exist.
    #[error("Rule not found: {0}")]
    NotFound(String),

    /// Returned when user-supplied input fails validation (e.g., bad regex pattern).
    #[error("Validation error: {0}")]
    Validation(String),

    /// Wraps OS clipboard access errors.
    #[error("Clipboard error: {0}")]
    Clipboard(String),

    /// Catch-all for unexpected internal failures.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        Self::Internal(error.to_string())
    }
}

impl From<sqlx::migrate::MigrateError> for AppError {
    fn from(error: sqlx::migrate::MigrateError) -> Self {
        Self::Database(error.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(error: serde_json::Error) -> Self {
        Self::Database(error.to_string())
    }
}

#[cfg(test)]
// Test assertions intentionally fail fast; production code still denies panicking helpers.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// Verify that `AppError` serializes to the expected discriminated-union shape.
    #[test]
    fn app_error_serializes_correctly() {
        let err = AppError::NotFound("rule 99".to_string());
        let json = serde_json::to_value(&err).expect("serialization must succeed in tests");
        assert_eq!(
            json["kind"], "notFound",
            "Missing rules must serialize with the notFound error kind expected by clients."
        );
        assert_eq!(
            json["message"], "rule 99",
            "A serialized missing-rule error must retain the specific rule identifier."
        );
    }

    #[test]
    fn validation_error_serializes_correctly() {
        let err = AppError::Validation("bad pattern".to_string());
        let json = serde_json::to_value(&err).expect("serialization must succeed in tests");
        assert_eq!(
            json["kind"], "validation",
            "Invalid input must serialize with the validation error kind."
        );
        assert_eq!(
            json["message"], "bad pattern",
            "A serialized validation error must retain its useful explanation."
        );
    }

    #[test]
    fn database_error_serializes_correctly() {
        let err = AppError::Database("connection refused".to_string());
        let json = serde_json::to_value(&err).expect("serialization must succeed in tests");
        assert_eq!(
            json["kind"], "database",
            "Database failures must serialize with a distinct database error kind."
        );
    }

    #[test]
    fn clipboard_and_internal_errors_round_trip_with_their_typed_messages()
    -> Result<(), serde_json::Error> {
        for (error, kind) in [
            (AppError::Clipboard("Clipboard locked".into()), "clipboard"),
            (AppError::Internal("Startup denied".into()), "internal"),
        ] {
            let json = serde_json::to_value(&error)?;
            assert_eq!(
                json["kind"], kind,
                "Clipboard and internal failures must keep their distinct error kinds: {kind}."
            );
            let decoded: AppError = serde_json::from_value(json)?;
            assert_eq!(
                decoded.to_string(),
                error.to_string(),
                "Serializing and reading an error back must preserve its human-readable explanation."
            );
        }
        Ok(())
    }
}
