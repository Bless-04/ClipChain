//! Pipeline orchestration service.
//!
//! Executes a sorted sequence of active rules against a clipboard text payload.
//! Business logic for *what* each rule does lives in [`super::transformer`].
//! This module only concerns itself with *ordering* and *filtering*.

use crate::{error::AppError, models::Rule, pipeline::transformer::apply_rule};

/// Runs every **active** rule in `rules` against `input` in ascending `order`.
///
/// Rules with `is_active == false` are silently skipped.
/// Rules are applied in-order: the output of rule N becomes the input of rule N+1.
///
/// # Errors
///
/// Returns the first [`AppError`] encountered. The pipeline is aborted at that
/// point; partial transformations are discarded.
pub fn run_pipeline(rules: &[Rule], input: &str) -> Result<String, AppError> {
    // Collect active rules and sort by order ascending.
    // We sort a Vec of references to avoid cloning the Rule data.
    let mut active: Vec<&Rule> = rules.iter().filter(|r| r.is_active()).collect();
    active.sort_by_key(|r| r.order());

    let mut text = input.to_owned();
    for rule in active {
        text = apply_rule(rule, &text)?;
    }
    Ok(text)
}

#[cfg(test)]
// Test assertions intentionally fail fast; production code still denies panicking helpers.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::models::{RegexRule, ReplaceRule, Rule, TrimRule};

    fn trim(order: i32, is_active: bool) -> Rule {
        Rule::Trim(TrimRule {
            id: 1,
            name: "trim".to_string(),
            order,
            is_active,
            trim_start: true,
            trim_end: true,
        })
    }

    fn replace(order: i32, target: &str, replacement: &str) -> Rule {
        Rule::Replace(ReplaceRule {
            id: 2,
            name: "replace".to_string(),
            order,
            is_active: true,
            target: target.to_string(),
            replacement: replacement.to_string(),
            match_case: true,
        })
    }

    fn regex(order: i32, pattern: &str, replacement: &str) -> Rule {
        Rule::Regex(RegexRule {
            id: 3,
            name: "regex".to_string(),
            order,
            is_active: true,
            pattern: pattern.to_string(),
            replacement: replacement.to_string(),
        })
    }

    #[test]
    fn empty_rule_set_returns_input_unchanged() {
        assert_eq!(
            run_pipeline(&[], "hello").unwrap(),
            "hello",
            "A pipeline with no rules must return the original text unchanged."
        );
    }

    #[test]
    fn empty_and_disabled_pipelines_still_enforce_input_limit() {
        let exact = "é".repeat(MAX_TEXT_BYTES / 2);
        assert_eq!(
            run_pipeline(&[], &exact).unwrap(),
            exact,
            "Even an empty pipeline must accept input exactly at the UTF-8 byte limit."
        );
        let excessive = exact + "é";
        assert!(
            matches!(run_pipeline(&[], &excessive), Err(AppError::Validation(_))),
            "An empty pipeline must still reject input beyond the byte limit."
        );
        assert!(
            matches!(
                run_pipeline(&[trim(0, false)], &excessive),
                Err(AppError::Validation(_))
            ),
            "Disabling every rule must not bypass the input size limit."
        );
    }

    #[test]
    fn inactive_rules_are_skipped() {
        let rules = vec![trim(0, false)];
        assert_eq!(
            run_pipeline(&rules, "  hello  ").unwrap(),
            "  hello  ",
            "Disabled rules must leave the input unchanged, including its whitespace."
        );
    }

    #[test]
    fn single_trim_rule() {
        let rules = vec![trim(0, true)];
        assert_eq!(
            run_pipeline(&rules, "  hello  ").unwrap(),
            "hello",
            "An enabled trim rule must remove surrounding whitespace."
        );
    }

    #[test]
    fn rules_execute_in_order_ascending() {
        // Order 1: trim whitespace → "hello world"
        // Order 2: replace "world" with "Rust"
        let rules = vec![replace(1, "world", "Rust"), trim(0, true)];
        assert_eq!(
            run_pipeline(&rules, "  hello world  ").unwrap(),
            "hello Rust",
            "Rules must execute by saved pipeline position, not by their order in the supplied list."
        );
    }

    #[test]
    fn pipeline_chains_three_rules() {
        let rules = vec![
            trim(0, true),
            replace(1, "secret", "[REDACTED]"),
            regex(2, r"\d+", "[NUM]"),
        ];
        let input = "  My secret is 12345  ";
        assert_eq!(
            run_pipeline(&rules, input).unwrap(),
            "My [REDACTED] is [NUM]",
            "Each rule must receive the previous rule's output so trimming and both redactions are retained."
        );
    }

    #[test]
    fn pipeline_aborts_on_first_error() {
        // A bad regex pattern should cause an error; replace rule at order 0 never runs.
        let rules = vec![
            Rule::Regex(RegexRule {
                id: 9,
                name: "bad".to_string(),
                order: 0,
                is_active: true,
                pattern: "(bad regex".to_string(),
                replacement: "x".to_string(),
            }),
            replace(1, "foo", "bar"),
        ];
        let err = run_pipeline(&rules, "foo bar").unwrap_err();
        assert!(
            matches!(err, AppError::Validation(_)),
            "The pipeline must return the first invalid-rule error instead of treating partial work as success."
        );
    }
}
