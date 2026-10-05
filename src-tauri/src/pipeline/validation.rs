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

#[cfg(test)]
mod tests {
    use super::validate_rule;
    use crate::{
        constants::validation,
        error::AppError,
        models::{RegexRule, ReplaceRule, Rule, TrimRule},
    };

    fn trim(name: &str) -> Rule {
        Rule::Trim(TrimRule {
            id: 0,
            order: 0,
            name: name.into(),
            is_active: true,
            trim_start: true,
            trim_end: true,
        })
    }

    #[test]
    fn identity_accepts_safe_boundaries_and_rejects_invalid_ids_or_positions() {
        for id in [0, validation::MAX_SAFE_ID] {
            let mut rule = trim("Valid");
            rule.set_identity(id, 0);
            assert!(validate_rule(&rule).is_ok());
        }
        for (id, order) in [(-1, 0), (validation::MAX_SAFE_ID + 1, 0), (0, -1)] {
            let mut rule = trim("Invalid");
            rule.set_identity(id, order);
            assert!(matches!(validate_rule(&rule), Err(AppError::Validation(_))));
        }
    }

    #[test]
    fn names_use_unicode_character_limits_and_reject_whitespace_only() {
        assert!(validate_rule(&trim(&"😀".repeat(validation::MAX_NAME_CHARS))).is_ok());
        for name in [
            String::new(),
            " \t\n\u{2003}".into(),
            "😀".repeat(validation::MAX_NAME_CHARS + 1),
        ] {
            assert!(matches!(
                validate_rule(&trim(&name)),
                Err(AppError::Validation(_))
            ));
        }
    }

    #[test]
    fn replace_fields_use_utf8_byte_limits_and_allow_empty_replacements() {
        let mut rule = ReplaceRule {
            id: 0,
            order: 0,
            name: "Replace".into(),
            is_active: true,
            target: "é".repeat(validation::MAX_FIELD_BYTES / 2),
            replacement: String::new(),
            match_case: false,
        };
        assert!(validate_rule(&Rule::Replace(rule.clone())).is_ok());
        rule.target.push('é');
        assert!(matches!(
            validate_rule(&Rule::Replace(rule.clone())),
            Err(AppError::Validation(_))
        ));
        rule.target = "x".into();
        rule.replacement = "é".repeat(validation::MAX_FIELD_BYTES / 2);
        assert!(validate_rule(&Rule::Replace(rule.clone())).is_ok());
        rule.replacement.push('é');
        assert!(matches!(
            validate_rule(&Rule::Replace(rule.clone())),
            Err(AppError::Validation(_))
        ));
        rule.target.clear();
        rule.replacement.clear();
        assert!(matches!(
            validate_rule(&Rule::Replace(rule)),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn regex_fields_are_bounded_even_for_disabled_rules() {
        let mut rule = RegexRule {
            id: 0,
            order: 0,
            name: "Regex".into(),
            is_active: false,
            pattern: String::new(),
            replacement: "é".repeat(validation::MAX_FIELD_BYTES / 2),
        };
        assert!(validate_rule(&Rule::Regex(rule.clone())).is_ok());
        rule.replacement.push('é');
        assert!(matches!(
            validate_rule(&Rule::Regex(rule.clone())),
            Err(AppError::Validation(_))
        ));
        rule.replacement.clear();
        rule.pattern = "a".repeat(validation::MAX_FIELD_BYTES + 1);
        assert!(matches!(
            validate_rule(&Rule::Regex(rule.clone())),
            Err(AppError::Validation(_))
        ));
        rule.pattern = "(?=unsupported)".into();
        assert!(matches!(
            validate_rule(&Rule::Regex(rule)),
            Err(AppError::Validation(_))
        ));
    }
}
