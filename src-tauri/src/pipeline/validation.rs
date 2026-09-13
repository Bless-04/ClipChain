//! Validate user-supplied rules before saving or previewing them.

use crate::{constants::validation, error::AppError, models::Rule};

/// Reject invalid identity, empty names, oversized fields, and invalid patterns.
///
/// # Errors
/// Returns a validation error explaining the invalid field.
pub fn validate_rule(rule: &Rule) -> Result<(), AppError> {
    if !(0..=validation::MAX_SAFE_ID).contains(&rule.id()) || rule.order() < 0 {
        return Err(AppError::Validation(
            "Invalid rule identity or position".into(),
        ));
    }
    if rule.name().trim().is_empty() || rule.name().chars().count() > validation::MAX_NAME_CHARS {
        return Err(AppError::Validation(format!(
            "Rule names must contain 1–{} characters",
            validation::MAX_NAME_CHARS
        )));
    }
    let fields: Vec<&str> = match rule {
        Rule::Trim(_) => vec![],
        Rule::Replace(rule) => {
            if rule.target.is_empty() {
                return Err(AppError::Validation(
                    "The text to replace must not be empty".into(),
                ));
            }
            vec![&rule.target, &rule.replacement]
        }
        Rule::Regex(rule) => vec![&rule.pattern, &rule.replacement],
    };
    if fields
        .iter()
        .any(|field| field.len() > validation::MAX_FIELD_BYTES)
    {
        return Err(AppError::Validation(
            "A rule field exceeds the 16 KiB limit".into(),
        ));
    }
    if let Rule::Regex(rule) = rule {
        regex::Regex::new(&rule.pattern)
            .map_err(|error| AppError::Validation(format!("Invalid regex pattern: {error}")))?;
    }
    Ok(())
}
