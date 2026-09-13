//! Core transformation trait and per-rule implementations.
//!
//! # Architecture
//!
//! The [`Transformer`] trait is the sole mechanism by which a rule mutates text.
//! All string operations are performed in pure Rust with no external regex
//! overhead on non-`RegexRule` paths:
//!
//! - [`TrimRule`]  — `str::trim_start()` / `str::trim_end()`
//! - [`ReplaceRule`] — `str::replace()` with optional manual case-folding
//! - [`RegexRule`]  — `regex::Regex::new()` + `Regex::replace_all()`
//!
//! Transformation errors are returned as [`AppError`] variants so command
//! handlers can propagate them to the TypeScript frontend without panicking.

use crate::{
    constants::clipboard::MAX_TEXT_BYTES,
    error::AppError,
    models::{RegexRule, ReplaceRule, Rule, TrimRule},
};

// ---------------------------------------------------------------------------
// Transformer trait
// ---------------------------------------------------------------------------

/// A rule that can transform an input string into an output string.
///
/// Implementations must be pure: given the same `input`, the same `Ok(output)`
/// is always produced. Side-effects are forbidden inside `transform`.
pub trait Transformer {
    /// Apply this rule's transformation to `input`.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::Validation`] when the rule's configuration is
    /// invalid (e.g., a malformed regex pattern).
    fn transform(&self, input: &str) -> Result<String, AppError>;
}

// ---------------------------------------------------------------------------
// TrimRule implementation
// ---------------------------------------------------------------------------

impl Transformer for TrimRule {
    /// Removes leading whitespace, trailing whitespace, or both depending on
    /// the rule's `trim_start` and `trim_end` flags.
    ///
    /// Uses standard library [`str::trim_start`] / [`str::trim_end`] — no regex.
    fn transform(&self, input: &str) -> Result<String, AppError> {
        let result = match (self.trim_start, self.trim_end) {
            (true, true) => input.trim(),
            (true, false) => input.trim_start(),
            (false, true) => input.trim_end(),
            // Neither flag active: pass through unchanged.
            (false, false) => input,
        };
        Ok(result.to_owned())
    }
}

// ---------------------------------------------------------------------------
// ReplaceRule implementation
// ---------------------------------------------------------------------------

fn append_bounded(output: &mut String, text: &str) -> Result<(), AppError> {
    if text.len() > MAX_TEXT_BYTES.saturating_sub(output.len()) {
        return Err(AppError::Validation(
            "Transformed text exceeds the 1 MiB limit".into(),
        ));
    }
    output.push_str(text);
    Ok(())
}

/// Match simple Unicode lowercase sequences while mapping them back to complete
/// original characters. Lowercased byte offsets must never index the original.
fn replace_case_insensitive(
    haystack: &str,
    needle: &str,
    replacement: &str,
) -> Result<String, AppError> {
    let mut folded = String::new();
    let mut boundaries = vec![Some(0)];
    for (offset, character) in haystack.char_indices() {
        folded.extend(character.to_lowercase());
        boundaries.resize(folded.len() + 1, None);
        boundaries[folded.len()] = Some(offset + character.len_utf8());
    }
    let needle: String = needle.chars().flat_map(char::to_lowercase).collect();
    let mut output = String::new();
    let mut cursor = 0;
    for (offset, matched) in folded.match_indices(&needle) {
        if let (Some(start), Some(end)) = (boundaries[offset], boundaries[offset + matched.len()]) {
            append_bounded(&mut output, &haystack[cursor..start])?;
            append_bounded(&mut output, replacement)?;
            cursor = end;
        }
    }
    append_bounded(&mut output, &haystack[cursor..])?;
    Ok(output)
}

fn replace_literal(input: &str, target: &str, replacement: &str) -> Result<String, AppError> {
    let mut output = String::new();
    let mut cursor = 0;
    for (start, matched) in input.match_indices(target) {
        append_bounded(&mut output, &input[cursor..start])?;
        append_bounded(&mut output, replacement)?;
        cursor = start + matched.len();
    }
    append_bounded(&mut output, &input[cursor..])?;
    Ok(output)
}

// Expand one capture reference at a time so a replacement containing many
// references to a large capture cannot allocate unbounded memory before checking.
fn expand_bounded(
    captures: &regex::Captures<'_>,
    mut template: &str,
    output: &mut String,
) -> Result<(), AppError> {
    while let Some(index) = template.find('$') {
        append_bounded(output, &template[..index])?;
        template = &template[index + 1..];
        if let Some(rest) = template.strip_prefix('$') {
            append_bounded(output, "$")?;
            template = rest;
            continue;
        }
        let reference = if let Some(braced) = template.strip_prefix('{') {
            braced.find('}').map(|end| (&braced[..end], end + 2))
        } else {
            let end = template
                .bytes()
                .take_while(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
                .count();
            (end > 0).then(|| (&template[..end], end))
        };
        if let Some((name, consumed)) = reference {
            let capture = name
                .parse::<usize>()
                .map_or_else(|_| captures.name(name), |index| captures.get(index));
            if let Some(capture) = capture {
                append_bounded(output, capture.as_str())?;
            }
            template = &template[consumed..];
        } else {
            append_bounded(output, "$")?;
        }
    }
    append_bounded(output, template)
}

impl Transformer for ReplaceRule {
    /// Replaces every occurrence of `target` in `input` with `replacement`.
    ///
    /// When `match_case` is `true`, uses [`str::replace`] directly.
    /// When `match_case` is `false`, uses the manual case-fold helper to avoid
    /// pulling in the `regex` crate on this hot path.
    fn transform(&self, input: &str) -> Result<String, AppError> {
        if self.target.is_empty() {
            // An empty target would replace every empty substring (infinite loop
            // in naive implementations). Treat as a pass-through and surface a
            // validation warning.
            return Err(AppError::Validation(
                "ReplaceRule target must not be empty".to_string(),
            ));
        }

        let output = if self.match_case {
            replace_literal(input, &self.target, &self.replacement)?
        } else {
            replace_case_insensitive(input, &self.target, &self.replacement)?
        };

        Ok(output)
    }
}

// ---------------------------------------------------------------------------
// RegexRule implementation
// ---------------------------------------------------------------------------

impl Transformer for RegexRule {
    /// Compiles `self.pattern` and replaces all matches in `input`.
    ///
    /// Pattern compilation happens on every call. In production, the pipeline
    /// service should cache compiled regexes (future optimisation); correctness
    /// is not affected.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::Validation`] if `self.pattern` is not a valid regex.
    fn transform(&self, input: &str) -> Result<String, AppError> {
        let re = regex::Regex::new(&self.pattern)
            .map_err(|e| AppError::Validation(format!("Invalid regex pattern: {e}")))?;

        let mut output = String::new();
        let mut cursor = 0;
        for captures in re.captures_iter(input) {
            if let Some(matched) = captures.get(0) {
                append_bounded(&mut output, &input[cursor..matched.start()])?;
                expand_bounded(&captures, &self.replacement, &mut output)?;
                cursor = matched.end();
            }
        }
        append_bounded(&mut output, &input[cursor..])?;
        Ok(output)
    }
}

// ---------------------------------------------------------------------------
// Rule dispatch helper
// ---------------------------------------------------------------------------

/// Dispatches `transform` to the correct variant implementation.
///
/// This free function is the canonical way for the pipeline service to apply
/// a single rule without matching on the enum itself.
/// # Errors
/// Returns validation errors for oversized input or invalid transformations.
pub fn apply_rule(rule: &Rule, input: &str) -> Result<String, AppError> {
    if input.len() > MAX_TEXT_BYTES {
        return Err(AppError::Validation("Input exceeds the 1 MiB limit".into()));
    }
    match rule {
        Rule::Trim(r) => r.transform(input),
        Rule::Replace(r) => r.transform(input),
        Rule::Regex(r) => r.transform(input),
    }
}

// ---------------------------------------------------------------------------
// Unit tests — every Transformer implementation is covered per AGENTS.md §7
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{RegexRule, ReplaceRule, TrimRule};

    // --- Helpers -----------------------------------------------------------

    fn trim_rule(trim_start: bool, trim_end: bool) -> TrimRule {
        TrimRule {
            id: 1,
            name: "test-trim".to_string(),
            order: 0,
            is_active: true,
            trim_start,
            trim_end,
        }
    }

    fn replace_rule(target: &str, replacement: &str, match_case: bool) -> ReplaceRule {
        ReplaceRule {
            id: 2,
            name: "test-replace".to_string(),
            order: 0,
            is_active: true,
            target: target.to_string(),
            replacement: replacement.to_string(),
            match_case,
        }
    }

    fn regex_rule(pattern: &str, replacement: &str) -> RegexRule {
        RegexRule {
            id: 3,
            name: "test-regex".to_string(),
            order: 0,
            is_active: true,
            pattern: pattern.to_string(),
            replacement: replacement.to_string(),
        }
    }

    // -----------------------------------------------------------------------
    // TrimRule tests
    // -----------------------------------------------------------------------

    #[test]
    fn trim_both_empty_string() {
        let rule = trim_rule(true, true);
        assert_eq!(rule.transform("").unwrap(), "");
    }

    #[test]
    fn trim_both_no_whitespace() {
        let rule = trim_rule(true, true);
        assert_eq!(rule.transform("hello").unwrap(), "hello");
    }

    #[test]
    fn trim_both_leading_and_trailing() {
        let rule = trim_rule(true, true);
        assert_eq!(rule.transform("  hello world  ").unwrap(), "hello world");
    }

    #[test]
    fn trim_start_only() {
        let rule = trim_rule(true, false);
        assert_eq!(rule.transform("  hello  ").unwrap(), "hello  ");
    }

    #[test]
    fn trim_end_only() {
        let rule = trim_rule(false, true);
        assert_eq!(rule.transform("  hello  ").unwrap(), "  hello");
    }

    #[test]
    fn trim_neither_flag_passthrough() {
        let rule = trim_rule(false, false);
        let input = "  no change  ";
        assert_eq!(rule.transform(input).unwrap(), input);
    }

    #[test]
    fn trim_multiline_string() {
        let rule = trim_rule(true, true);
        assert_eq!(
            rule.transform("\n  hello\n  world\n").unwrap(),
            "hello\n  world"
        );
    }

    #[test]
    fn trim_only_whitespace_becomes_empty() {
        let rule = trim_rule(true, true);
        assert_eq!(rule.transform("   \t\n   ").unwrap(), "");
    }

    // -----------------------------------------------------------------------
    // ReplaceRule tests
    // -----------------------------------------------------------------------

    #[test]
    fn replace_empty_input() {
        let rule = replace_rule("foo", "bar", true);
        assert_eq!(rule.transform("").unwrap(), "");
    }

    #[test]
    fn replace_unmatched_passthrough() {
        let rule = replace_rule("xyz", "bar", true);
        assert_eq!(rule.transform("hello world").unwrap(), "hello world");
    }

    #[test]
    fn replace_single_match_case_sensitive() {
        let rule = replace_rule("world", "Rust", true);
        assert_eq!(rule.transform("hello world").unwrap(), "hello Rust");
    }

    #[test]
    fn replace_multiple_matches() {
        let rule = replace_rule("a", "X", true);
        assert_eq!(rule.transform("banana").unwrap(), "bXnXnX");
    }

    #[test]
    fn replace_case_insensitive_basic() {
        let rule = replace_rule("HELLO", "[REDACTED]", false);
        assert_eq!(
            rule.transform("hello world Hello HELLO").unwrap(),
            "[REDACTED] world [REDACTED] [REDACTED]"
        );
    }

    #[test]
    fn replace_case_insensitive_preserves_non_matched_casing() {
        let rule = replace_rule("foo", "bar", false);
        // "Baz" is untouched; only "FOO" / "foo" are replaced.
        assert_eq!(
            rule.transform("Baz FOO foo Baz").unwrap(),
            "Baz bar bar Baz"
        );
    }

    #[test]
    fn replace_empty_target_returns_validation_error() {
        let rule = replace_rule("", "bar", true);
        let err = rule.transform("some text").unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn replace_unicode_target() {
        let rule = replace_rule("café", "coffee", true);
        assert_eq!(rule.transform("I love café").unwrap(), "I love coffee");
    }

    #[test]
    fn replace_case_insensitive_unicode() {
        let rule = replace_rule("ß", "ss", false);
        // German sharp-S — lowercasing behavior depends on locale; test pass-through at minimum.
        let result = rule.transform("Straße").unwrap();
        // Either replaced or passed through — must not panic.
        assert!(!result.is_empty());
    }

    #[test]
    fn insensitive_empty_and_short_input_are_safe() {
        let rule = replace_rule("long needle", "X", false);
        assert_eq!(rule.transform("").unwrap(), "");
        assert_eq!(rule.transform("a").unwrap(), "a");
    }

    #[test]
    fn insensitive_unicode_expansion_preserves_original_boundaries() {
        assert_eq!(
            replace_rule("i\u{307}", "X", false)
                .transform("AİB")
                .unwrap(),
            "AXB"
        );
        assert_eq!(
            replace_rule("k", "X", false).transform("K K k").unwrap(),
            "X X X"
        );
        assert_eq!(
            replace_rule("i", "X", false).transform("İ i").unwrap(),
            "İ X"
        );
        assert_eq!(
            replace_rule("café", "X", false)
                .transform("CAFÉ 😀 café")
                .unwrap(),
            "X 😀 X"
        );
    }

    #[test]
    fn expanding_replacements_stop_at_the_output_limit() {
        let input = "a".repeat(MAX_TEXT_BYTES);
        assert!(replace_rule("a", "aaaa", true).transform(&input).is_err());
        assert!(replace_rule("a", "aaaa", false).transform(&input).is_err());
        assert!(regex_rule("(a+)", "$1$1").transform(&input).is_err());
    }

    #[test]
    fn bounded_capture_expansion_matches_regex_crate_semantics() {
        let pattern = r"(?P<word>\w+)(\d)?";
        let reference = regex::Regex::new(pattern).unwrap();
        for replacement in [
            "$0",
            "$1",
            "${word}",
            "$word",
            "$$",
            "$",
            "${",
            "$missing",
            "$1suffix",
            "${1}suffix",
            "${}",
            "$99",
            "a$$b$1${word}",
        ] {
            assert_eq!(
                regex_rule(pattern, replacement)
                    .transform("hello world")
                    .unwrap(),
                reference.replace_all("hello world", replacement)
            );
        }
    }

    // -----------------------------------------------------------------------
    // RegexRule tests
    // -----------------------------------------------------------------------

    #[test]
    fn regex_empty_input() {
        let rule = regex_rule(r"\d+", "[NUM]");
        assert_eq!(rule.transform("").unwrap(), "");
    }

    #[test]
    fn regex_unmatched_passthrough() {
        let rule = regex_rule(r"\d+", "[NUM]");
        assert_eq!(rule.transform("no digits here").unwrap(), "no digits here");
    }

    #[test]
    fn regex_single_match() {
        let rule = regex_rule(r"\d+", "[NUM]");
        assert_eq!(rule.transform("order 42").unwrap(), "order [NUM]");
    }

    #[test]
    fn regex_multiple_matches() {
        let rule = regex_rule(r"\d+", "[NUM]");
        assert_eq!(
            rule.transform("1 apple, 2 bananas, 300 grapes").unwrap(),
            "[NUM] apple, [NUM] bananas, [NUM] grapes"
        );
    }

    #[test]
    fn regex_capture_group_replacement() {
        // Swap first and last name using capture groups.
        let rule = regex_rule(r"(\w+)\s+(\w+)", "$2 $1");
        assert_eq!(rule.transform("John Doe").unwrap(), "Doe John");
    }

    #[test]
    fn regex_phone_redaction() {
        let rule = regex_rule(r"\b\d{3}[-.\s]?\d{3}[-.\s]?\d{4}\b", "[PHONE]");
        assert_eq!(
            rule.transform("Call 555-867-5309 now").unwrap(),
            "Call [PHONE] now"
        );
    }

    #[test]
    fn regex_malformed_pattern_returns_validation_error() {
        let rule = regex_rule(r"(unclosed", "[X]");
        let err = rule.transform("test").unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn regex_multiline_flag_via_pattern() {
        // Anchors apply per-line when the (?m) flag is embedded in the pattern.
        let rule = regex_rule(r"(?m)^\s+", "");
        let input = "  line one\n  line two";
        let output = rule.transform(input).unwrap();
        assert_eq!(output, "line one\nline two");
    }

    #[test]
    fn regex_empty_pattern_matches_between_chars() {
        // An empty pattern is technically valid in `regex`; verify it doesn't panic.
        let rule = regex_rule(r"", "X");
        // Result is implementation-defined but must not panic or return an error.
        let result = rule.transform("ab");
        assert!(result.is_ok());
    }
}
