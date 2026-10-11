//! End-to-end console tests use disposable databases and never open a clipboard.
#![deny(clippy::pedantic, clippy::unwrap_used, clippy::expect_used)]

use clipchain_lib::{
    clipboard::ownership::ClipboardLease,
    constants::clipboard::MAX_TEXT_BYTES,
    db,
    models::{AppSettings, RegexRule, ReplaceRule, Rule, TrimRule},
};
use serde_json::Value;
use std::{
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn cli(path: &Path, args: &[&str], input: &[u8]) -> Result<Output, Box<dyn std::error::Error>> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_clipchain-cli"))
        .arg("--db")
        .arg(path)
        .args(args)
        .env("XDG_DATA_HOME", path.parent().ok_or("database parent")?)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stdin = child.stdin.take().ok_or("stdin unavailable")?;
    stdin.write_all(input)?;
    drop(stdin);
    Ok(child.wait_with_output()?)
}

fn success(output: &Output) {
    assert!(
        output.status.success(),
        "The CLI command should have succeeded; captured stderr follows. ({})",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stderr, b"",
        "A successful CLI command must keep stderr empty so scripts can distinguish failures."
    );
}

fn json(output: &Output) -> Result<Value, serde_json::Error> {
    serde_json::from_slice(&output.stdout)
}

async fn seed(path: &Path) -> Result<(), clipchain_lib::error::AppError> {
    let pool = db::open(path).await?;
    for rule in [
        Rule::Trim(TrimRule {
            id: 0,
            order: 0,
            name: "Trim".into(),
            is_active: true,
            trim_start: true,
            trim_end: true,
        }),
        Rule::Replace(ReplaceRule {
            id: 0,
            order: 0,
            name: "Replace".into(),
            is_active: true,
            target: "copy".into(),
            replacement: "item42".into(),
            match_case: true,
        }),
        Rule::Regex(RegexRule {
            id: 0,
            order: 0,
            name: "Regex".into(),
            is_active: true,
            pattern: "^item(\\d+)$".into(),
            replacement: "number $1".into(),
        }),
        Rule::Replace(ReplaceRule {
            id: 0,
            order: 0,
            name: "Disabled".into(),
            is_active: false,
            target: "number".into(),
            replacement: "wrong".into(),
            match_case: true,
        }),
    ] {
        db::rules::save(&pool, rule).await?;
    }
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn stored_pipeline_transforms_arguments_and_piped_input_without_history() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("test.db");
    seed(&path).await?;
    for (args, input) in [
        (vec!["transform", "  copy  "], b"".as_slice()),
        (vec!["transform"], b"  copy\n".as_slice()),
    ] {
        let output = cli(&path, &args, input)?;
        success(&output);
        assert_eq!(
            output.stdout, b"number 42",
            "Positional and piped input must produce the same saved Trim, Replace, and Regex result."
        );
    }
    let output = cli(&path, &["transform", " copy ", "--dry-run"], b"")?;
    success(&output);
    let report = json(&output)?;
    assert_eq!(
        report["changed"], true,
        "The dry-run report must say that the saved pipeline changed the input."
    );
    assert_eq!(
        report["transformedText"], "number 42",
        "Dry-run output must contain the final transformed text."
    );
    assert_eq!(
        report["activeRuleCount"], 3,
        "Dry-run counts must exclude disabled rules."
    );
    let pool = db::open(&path).await?;
    assert!(
        db::history::get_all(&pool).await?.is_empty(),
        "Text transforms and dry runs must not add clipboard history."
    );
    pool.close().await;
    Ok(())
}

#[test]
fn draft_previews_and_adhoc_transforms_do_not_open_database() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("unopened.db");
    for (args, input, expected) in [
        (
            vec!["rules", "preview", "--type", "trim", "  😀 value\n"],
            b"".as_slice(),
            "😀 value",
        ),
        (
            vec![
                "rules",
                "preview",
                "--type",
                "replace",
                "--target",
                "é",
                "--replacement",
                "",
                "--ignore-case",
            ],
            "Été".as_bytes(),
            "t",
        ),
        (
            vec![
                "rules",
                "preview",
                "--type",
                "regex",
                "--pattern",
                r"(\d+)",
                "--replacement",
                "[$1]",
            ],
            b"line 42\n",
            "line [42]\n",
        ),
        (
            vec!["transform", "--type", "trim", "--trim-start", "false"],
            b"  text \n",
            "  text",
        ),
        (
            vec![
                "transform",
                "--type",
                "replace",
                "--target",
                "x",
                "--replacement",
                "y",
                "x",
            ],
            b"",
            "y",
        ),
    ] {
        let output = cli(&path, &args, input)?;
        success(&output);
        assert_eq!(
            String::from_utf8(output.stdout)?,
            expected,
            "Draft processing must produce the expected text for this command: {args:?}."
        );
    }
    assert!(
        !path.exists(),
        "Unsaved previews must not create or open the selected database."
    );
    Ok(())
}

#[test]
fn invalid_drafts_and_unconfirmed_mutations_fail_without_creating_database() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("unopened.db");
    for args in [
        vec![
            "rules",
            "preview",
            "--type",
            "regex",
            "--pattern",
            "(?=x)",
            "--replacement",
            "y",
            "x",
        ],
        vec![
            "rules",
            "preview",
            "--type",
            "replace",
            "--target",
            "",
            "--replacement",
            "x",
            "x",
        ],
        vec!["rules", "preview", "--type", "trim", "--target", "x", "x"],
        vec!["rules", "preview", "text"],
        vec!["transform", "--target", "x", "x"],
        vec!["history", "clear"],
        vec!["rules", "delete", "1"],
        vec!["settings", "set"],
    ] {
        let output = cli(&path, &args, b"")?;
        assert!(
            !output.status.success(),
            "Invalid drafts and unconfirmed destructive commands must fail: {args:?}."
        );
        assert_eq!(
            output.stdout, b"",
            "Rejected commands must leave stdout empty so scripts do not consume a false result."
        );
        let error: Value = serde_json::from_slice(&output.stderr)?;
        assert_eq!(
            error["error"]["kind"], "validation",
            "Rejected draft or confirmation input must return a typed validation error."
        );
    }
    assert!(
        !path.exists(),
        "Validation must happen before creating a database."
    );
    Ok(())
}

#[tokio::test]
async fn empty_pipeline_preserves_multiline_utf8_and_enforces_byte_limits() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("test.db");
    let input = "  😀\r\nsecond\n";
    let output = cli(&path, &["transform"], input.as_bytes())?;
    success(&output);
    assert_eq!(
        output.stdout,
        input.as_bytes(),
        "An empty pipeline must preserve Unicode, CRLF, and trailing newlines byte for byte."
    );
    let exact = "é".repeat(MAX_TEXT_BYTES / 2);
    let output = cli(&path, &["transform"], exact.as_bytes())?;
    success(&output);
    assert_eq!(
        output.stdout,
        exact.as_bytes(),
        "UTF-8 input exactly at the byte limit must be accepted without truncation."
    );
    for input in [(exact + "é").into_bytes(), vec![0xff]] {
        let output = cli(&path, &["transform"], &input)?;
        assert!(
            !output.status.success(),
            "Oversized or non-UTF-8 input must fail even when there are no saved rules."
        );
        assert_eq!(
            output.stdout, b"",
            "Invalid input must not produce partial output."
        );
        assert!(
            String::from_utf8(output.stderr)?.contains("validation"),
            "Invalid text input must be reported as a validation error on stderr."
        );
    }
    let output = cli(&path, &["transform"], b"")?;
    success(&output);
    assert_eq!(
        output.stdout, b"",
        "Empty stdin is valid and must not produce an added newline."
    );
    Ok(())
}

#[test]
fn excessive_output_fails_without_printing_partial_transformation() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("unopened.db");
    let output = cli(
        &path,
        &[
            "transform",
            "--type",
            "replace",
            "--target",
            "a",
            "--replacement",
            "aa",
        ],
        &vec![b'a'; MAX_TEXT_BYTES],
    )?;
    assert!(
        !output.status.success(),
        "A replacement that expands beyond the output limit must fail."
    );
    assert_eq!(
        output.stdout, b"",
        "Oversized output must be rejected before any partial result reaches stdout."
    );
    assert!(
        String::from_utf8(output.stderr)?.contains("validation"),
        "Excessive output must be explained as a validation error."
    );
    assert!(
        !path.exists(),
        "A failed ad-hoc transform must not create a database."
    );
    Ok(())
}

#[tokio::test]
// Keep this sequential management scenario together; explanatory assertion
// messages account for the extra lines without adding test behavior.
#[allow(clippy::too_many_lines)]
async fn management_commands_preserve_identity_order_startup_and_unrelated_data() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("test.db");
    seed(&path).await?;
    let pool = db::open(&path).await?;
    db::settings::save(
        &pool,
        &AppSettings {
            launch_at_startup: true,
            ..Default::default()
        },
    )
    .await?;
    db::history::insert(&pool, "before1", "after1").await?;
    db::history::insert(&pool, "before2", "after2").await?;
    pool.close().await;
    let output = cli(&path, &["rules", "list", "--json"], b"")?;
    success(&output);
    let rules = json(&output)?;
    assert_eq!(
        rules.as_array().ok_or("rules array")?.len(),
        4,
        "Listing rules must include all saved rules, including disabled ones."
    );
    assert_eq!(
        rules[3]["data"]["isActive"], false,
        "Listing rules must preserve the saved disabled state."
    );
    let output = cli(&path, &["rules", "reorder", "3", "2", "1", "4"], b"")?;
    success(&output);
    assert_eq!(
        json(&output)?[0]["data"]["id"],
        3,
        "The first ID supplied to reorder must become the first pipeline rule."
    );
    let failed = cli(&path, &["rules", "reorder", "3", "3"], b"")?;
    assert!(
        !failed.status.success(),
        "A reorder containing duplicate IDs must fail."
    );
    let mut edited = rules[0].clone();
    edited["data"]["name"] = "Edited trim".into();
    let file = directory.path().join("rule.json");
    std::fs::write(&file, serde_json::to_vec(&edited)?)?;
    let output = cli(
        &path,
        &["rules", "save", "--file", file.to_str().ok_or("file path")?],
        b"",
    )?;
    success(&output);
    assert_eq!(
        json(&output)?["data"]["order"],
        2,
        "Editing a reordered rule must preserve its new pipeline position."
    );
    let output = cli(
        &path,
        &[
            "settings",
            "set",
            "--active",
            "false",
            "--record-history",
            "false",
        ],
        b"",
    )?;
    success(&output);
    assert_eq!(
        json(&output)?["launchAtStartup"],
        true,
        "Changing monitor settings from the CLI must preserve the startup preference."
    );
    assert_eq!(
        json(&output)?["isActive"],
        false,
        "The CLI must return the newly paused monitoring state."
    );
    let output = cli(&path, &["history", "list", "--limit", "1", "--json"], b"")?;
    success(&output);
    assert_eq!(
        json(&output)?[0]["originalText"],
        "before2",
        "Limited history must return the newest retained entry."
    );
    assert_eq!(
        json(&output)?.as_array().ok_or("history array")?.len(),
        1,
        "The requested history limit must cap the number of returned entries."
    );
    success(&cli(&path, &["history", "clear", "--yes"], b"")?);
    success(&cli(&path, &["rules", "delete", "4", "--yes"], b"")?);
    let pool = db::open(&path).await?;
    assert_eq!(
        db::rules::get_all(&pool).await?.len(),
        3,
        "Deleting one rule must preserve the other three."
    );
    assert!(
        db::history::get_all(&pool).await?.is_empty(),
        "A confirmed clear must remove all history entries."
    );
    let settings = db::settings::get(&pool).await?;
    assert!(
        settings.launch_at_startup,
        "CLI rule and history mutations must not change startup preference."
    );
    assert!(
        !settings.is_active,
        "The paused monitoring setting must survive later rule and history mutations."
    );
    assert!(
        !settings.record_history,
        "The disabled recording setting must survive later rule and history mutations."
    );
    pool.close().await;
    Ok(())
}

#[test]
fn lock_probe_child() {
    if let Some(path) = std::env::var_os("CLIPCHAIN_TEST_LOCK") {
        let lock = std::path::PathBuf::from(path);
        assert!(
            ClipboardLease::acquire(&lock, &lock.with_extension("db")).is_err(),
            "A separate process must be denied ownership while the parent holds the clipboard lock."
        );
    }
}

#[test]
fn clipboard_ownership_rejects_a_separate_process() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("clipboard.lock");
    let _lease = ClipboardLease::acquire(&path, &directory.path().join("test.db"))?;
    let output = Command::new(std::env::current_exe()?)
        .args(["--exact", "lock_probe_child", "--nocapture"])
        .env("CLIPCHAIN_TEST_LOCK", &path)
        .output()?;
    assert!(
        output.status.success(),
        "The child lock probe should pass only when competing ownership is rejected; child output follows. ({})",
        String::from_utf8_lossy(&output.stdout)
    );
    Ok(())
}

#[test]
fn help_version_and_argument_errors_do_not_open_a_database() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("unopened.db");
    for args in [
        vec!["--help"],
        vec!["--version"],
        vec!["rules", "preview", "--help"],
    ] {
        success(&cli(&path, &args, b"")?);
    }
    let output = cli(&path, &["history", "list", "--limit", "501"], b"")?;
    assert_eq!(
        output.status.code(),
        Some(2),
        "An out-of-range history limit must use parser error exit code 2."
    );
    assert!(
        !path.exists(),
        "Help, version, and parser failures must not create a database."
    );
    Ok(())
}

// Linux supports a child-only data-directory override without changing HOME.
// Windows/macOS use native directory APIs; their lock tests inject temporary paths.
#[cfg(target_os = "linux")]
#[test]
fn clipboard_commands_reject_live_ownership_before_opening_clipboard_or_database() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("unopened.db");
    let lock = directory
        .path()
        .join("com.blessing.clipchain/clipboard.lock");
    let _lease = ClipboardLease::acquire(&lock, &directory.path().join("owner.db"))?;
    for args in [vec!["clip"], vec!["daemon"]] {
        let output = cli(&path, &args, b"")?;
        assert_eq!(
            output.status.code(),
            Some(1),
            "A conflicting clipboard owner must cause an operational failure with exit code 1."
        );
        let error: Value = serde_json::from_slice(&output.stderr)?;
        assert_eq!(
            error["error"]["kind"], "clipboard",
            "An ownership conflict must be classified as a clipboard error."
        );
        assert!(
            error["error"]["message"]
                .as_str()
                .ok_or("error message")?
                .contains("consumer is active"),
            "The error must explain that another clipboard consumer is already active."
        );
        assert_eq!(
            output.stdout, b"",
            "An ownership failure must not print a successful clipboard result."
        );
    }
    assert!(
        !path.exists(),
        "Clipboard ownership must be checked before creating the requested database."
    );
    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn status_reports_selected_database_and_live_owner_and_ignores_stale_health() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("test.db");
    seed(&path).await?;
    let lock = directory
        .path()
        .join("com.blessing.clipchain/clipboard.lock");
    let lease = ClipboardLease::acquire(&lock, &directory.path().join("other.db"))?;
    let output = cli(&path, &["status", "--json"], b"")?;
    success(&output);
    let status = json(&output)?;
    assert_eq!(
        status["databasePath"],
        path.to_str().ok_or("database path")?,
        "Status must identify the database selected by --db."
    );
    assert_eq!(
        status["ruleCount"], 4,
        "Status must include all saved rules in its total count."
    );
    assert_eq!(
        status["activeRuleCount"], 3,
        "Status must count only enabled rules as active."
    );
    assert_eq!(
        status["worker"]["hasOwner"], true,
        "Status must detect the process that currently owns the clipboard lock."
    );
    assert_eq!(
        status["worker"]["databasePath"],
        directory
            .path()
            .join("other.db")
            .to_str()
            .ok_or("owner path")?,
        "Status must distinguish the owner's database from the database being inspected."
    );
    drop(lease);
    let output = cli(&path, &["status", "--json"], b"")?;
    success(&output);
    assert_eq!(
        json(&output)?["worker"]["hasOwner"],
        false,
        "Status must report no owner once the ownership lease is released."
    );
    assert_eq!(
        json(&output)?["worker"]["status"],
        Value::Null,
        "Status must ignore stale health data after the owning process releases its lock."
    );
    Ok(())
}
