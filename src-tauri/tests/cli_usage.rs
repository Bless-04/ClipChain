//! Public CLI compatibility fixtures. Keep argv/help expectations literal and
//! independent of Clap definitions: an intentional breaking change needs review.
//! Only the console executable runs; all persistent fixtures are disposable.
#![deny(clippy::pedantic, clippy::unwrap_used, clippy::expect_used)]

#[path = "../bin/cli/args.rs"]
mod args;

use args::{Cli, Command, HistoryCommand, RulesCommand, SettingsCommand};
use clap::{Parser, error::ErrorKind};
use clipchain_lib::{db, models::Rule};
use serde_json::{Value, json};
use std::{
    io::Write,
    path::Path,
    process::{Command as Process, Output, Stdio},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

// This projection checks the selected operation and its values, not Clap's
// internal argument IDs. The separate, literal argv below is the public contract.
fn operation(command: Command) -> Value {
    match command {
        Command::Transform { text, dry_run, .. } => json!(["transform", text, dry_run]),
        Command::Clip => json!(["clip"]),
        Command::Daemon => json!(["daemon"]),
        Command::Rules { command } => match command {
            RulesCommand::List { json } => json!(["rules list", json]),
            RulesCommand::Preview { input, .. } => json!(["rules preview", input]),
            RulesCommand::Save { file } => json!(["rules save", file]),
            RulesCommand::Delete { id, yes } => json!(["rules delete", id, yes]),
            RulesCommand::Reorder { ids } => json!(["rules reorder", ids]),
        },
        Command::History { command } => match command {
            HistoryCommand::List { limit, json } => json!(["history list", limit, json]),
            HistoryCommand::Clear { yes } => json!(["history clear", yes]),
        },
        Command::Settings { command } => match command {
            SettingsCommand::Get { json } => json!(["settings get", json]),
            SettingsCommand::Set {
                active,
                record_history,
            } => {
                json!(["settings set", active, record_history])
            }
        },
        Command::Status { json } => json!(["status", json]),
    }
}

#[test]
fn published_commands_arguments_and_defaults() -> TestResult {
    let fixtures: &[(&[&str], Value)] = &[
        (&["transform"], json!(["transform", null, false])),
        (
            &["transform", "", "--dry-run"],
            json!(["transform", "", true]),
        ),
        (
            &["transform", "--", "-text"],
            json!(["transform", "-text", false]),
        ),
        (&["clip"], json!(["clip"])),
        (&["daemon"], json!(["daemon"])),
        (&["rules", "list"], json!(["rules list", false])),
        (&["rules", "list", "--json"], json!(["rules list", true])),
        (
            &["rules", "preview", "--type", "trim"],
            json!(["rules preview", null]),
        ),
        (
            &["rules", "preview", "--type", "trim", "--", "-input"],
            json!(["rules preview", "-input"]),
        ),
        (
            &["rules", "save", "--file", "rule file.json"],
            json!(["rules save", "rule file.json"]),
        ),
        (
            &["rules", "delete", "42"],
            json!(["rules delete", 42, false]),
        ),
        (
            &["rules", "delete", "42", "--yes"],
            json!(["rules delete", 42, true]),
        ),
        (&["rules", "reorder", "9"], json!(["rules reorder", [9]])),
        (
            &["rules", "reorder", "3", "1", "2"],
            json!(["rules reorder", [3, 1, 2]]),
        ),
        (&["history", "list"], json!(["history list", 20, false])),
        (
            &["history", "list", "--limit", "1", "--json"],
            json!(["history list", 1, true]),
        ),
        (
            &["history", "list", "--limit", "500"],
            json!(["history list", 500, false]),
        ),
        (&["history", "clear"], json!(["history clear", false])),
        (
            &["history", "clear", "--yes"],
            json!(["history clear", true]),
        ),
        (&["settings", "get"], json!(["settings get", false])),
        (
            &["settings", "get", "--json"],
            json!(["settings get", true]),
        ),
        (&["settings", "set"], json!(["settings set", null, null])),
        (
            &["settings", "set", "--active", "false"],
            json!(["settings set", false, null]),
        ),
        (
            &["settings", "set", "--record-history", "true"],
            json!(["settings set", null, true]),
        ),
        (
            &[
                "settings",
                "set",
                "--active",
                "true",
                "--record-history",
                "false",
            ],
            json!(["settings set", true, false]),
        ),
        (&["status"], json!(["status", false])),
        (&["status", "--json"], json!(["status", true])),
    ];
    for (argv, expected) in fixtures {
        assert_operation_and_database_placement(argv, expected)?;
    }
    Ok(())
}

fn assert_operation_and_database_placement(argv: &[&str], expected: &Value) -> TestResult {
    let parsed = Cli::try_parse_from(std::iter::once("clipchain-cli").chain(argv.iter().copied()))?;
    assert!(parsed.db.is_none(), "{argv:?}");
    assert_eq!(operation(parsed.command), *expected, "{argv:?}");
    // Include the boundary between a group and its leaf, but never insert a
    // global option after the positional '--' terminator.
    let end = argv
        .iter()
        .position(|arg| *arg == "--")
        .unwrap_or(argv.len());
    for index in [0, 1, end] {
        let mut with_db = vec!["clipchain-cli"];
        with_db.extend_from_slice(&argv[..index]);
        with_db.extend_from_slice(&["--db", "custom data/clipchain.db"]);
        with_db.extend_from_slice(&argv[index..]);
        let parsed = Cli::try_parse_from(with_db)?;
        assert_eq!(
            parsed.db.as_deref(),
            Some(Path::new("custom data/clipchain.db"))
        );
        assert_eq!(operation(parsed.command), *expected, "{argv:?} at {index}");
    }
    Ok(())
}

#[test]
fn draft_options_keep_values_and_defaults_for_both_entry_points() -> TestResult {
    let fixtures: &[(&[&str], Value)] = &[
        (
            &["--type", "trim"],
            json!({"trimStart": true, "trimEnd": true}),
        ),
        (
            &[
                "--type",
                "trim",
                "--trim-start",
                "false",
                "--trim-end",
                "true",
            ],
            json!({"trimStart": false, "trimEnd": true}),
        ),
        (
            &[
                "--type",
                "trim",
                "--trim-start",
                "true",
                "--trim-end",
                "false",
            ],
            json!({"trimStart": true, "trimEnd": false}),
        ),
        (
            &["--type", "replace", "--target", "x", "--replacement", "-y"],
            json!({"target": "x", "replacement": "-y", "matchCase": true}),
        ),
        (
            &[
                "--type",
                "replace",
                "--target",
                "é",
                "--replacement",
                "",
                "--ignore-case",
            ],
            json!({"target": "é", "replacement": "", "matchCase": false}),
        ),
        (
            &[
                "--type",
                "regex",
                "--pattern",
                "(\\d+)",
                "--replacement",
                "[$1]",
            ],
            json!({"pattern": "(\\d+)", "replacement": "[$1]"}),
        ),
        (
            &["--type", "regex", "--pattern", "x", "--replacement", ""],
            json!({"pattern": "x", "replacement": ""}),
        ),
    ];
    for prefix in [
        vec!["clipchain-cli", "transform"],
        vec!["clipchain-cli", "rules", "preview"],
    ] {
        for (options, expected) in fixtures {
            let parsed =
                Cli::try_parse_from(prefix.iter().copied().chain(options.iter().copied()))?;
            let (Command::Transform { draft, .. }
            | Command::Rules {
                command: RulesCommand::Preview { draft, .. },
            }) = parsed.command
            else {
                return Err("wrong draft command".into());
            };
            let rule = serde_json::to_value(draft.rule()?.ok_or("missing draft")?)?;
            assert_eq!(rule["type"], options[1]);
            for (key, value) in expected.as_object().ok_or("expected draft fields")? {
                assert_eq!(rule["data"][key], *value, "{prefix:?} {options:?}: {key}");
            }
        }
    }
    let parsed = Cli::try_parse_from(["clipchain-cli", "transform"])?;
    let Command::Transform { draft, .. } = parsed.command else {
        return Err("wrong command".into());
    };
    assert!(draft.rule()?.is_none());
    Ok(())
}

#[test]
fn parser_rejects_unknown_missing_extra_and_out_of_range_arguments() {
    // Native operations are parsed only, including malformed invocations.
    let fixtures: &[&[&str]] = &[
        &[],
        &["unknown"],
        &["--unknown"],
        &["--db"],
        &["rules"],
        &["history"],
        &["settings"],
        &["rules", "unknown"],
        &["history", "unknown"],
        &["settings", "unknown"],
        &["transform", "a", "b"],
        &["transform", "--dryrun"],
        &["transform", "--type"],
        &["transform", "--type", "wrap"],
        &["transform", "--type", "TRIM"],
        &["transform", "--target"],
        &["transform", "--pattern"],
        &["transform", "--replacement"],
        &["transform", "--trim-start"],
        &["transform", "--trim-end", "yes"],
        &["clip", "extra"],
        &["clip", "--json"],
        &["daemon", "extra"],
        &["daemon", "--json"],
        &["rules", "list", "extra"],
        &["rules", "list", "--unknown"],
        &["rules", "preview", "a", "b"],
        &["rules", "preview", "--type", "wrap"],
        &["rules", "save"],
        &["rules", "save", "--file"],
        &["rules", "save", "--file", "a", "b"],
        &["rules", "delete"],
        &["rules", "delete", "x"],
        &["rules", "delete", "1", "2"],
        &["rules", "reorder"],
        &["rules", "reorder", "1", "x"],
        &["history", "list", "--limit"],
        &["history", "list", "--limit", "0"],
        &["history", "list", "--limit", "501"],
        &["history", "list", "--limit", "1.5"],
        &["history", "list", "extra"],
        &["history", "clear", "extra"],
        &["settings", "get", "extra"],
        &["settings", "set", "extra"],
        &["settings", "set", "--active"],
        &["settings", "set", "--record-history"],
        &["settings", "set", "--active", "1"],
        &["settings", "set", "--record-history", "yes"],
        &["status", "extra"],
        &["status", "--unknown"],
    ];
    for argv in fixtures {
        let result =
            Cli::try_parse_from(std::iter::once("clipchain-cli").chain(argv.iter().copied()));
        let Err(error) = result else {
            panic!("unexpectedly accepted {argv:?}")
        };
        assert_eq!(error.exit_code(), 2, "{argv:?}");
        assert!(error.use_stderr(), "{argv:?}");
    }
}

#[test]
fn help_and_version_short_and_long_aliases_parse_without_dispatch() {
    for (flag, kind) in [
        ("-h", ErrorKind::DisplayHelp),
        ("--help", ErrorKind::DisplayHelp),
        ("-V", ErrorKind::DisplayVersion),
        ("--version", ErrorKind::DisplayVersion),
    ] {
        let Err(error) = Cli::try_parse_from(["clipchain-cli", flag]) else {
            panic!("{flag} dispatched")
        };
        assert_eq!(error.kind(), kind);
        assert_eq!(error.exit_code(), 0);
        assert!(!error.use_stderr());
    }
}

fn execute(path: &Path, argv: &[&str], input: &[u8]) -> Result<Output, Box<dyn std::error::Error>> {
    // A test edit cannot accidentally start a native worker through this helper.
    assert!(!argv.iter().any(|arg| matches!(*arg, "clip" | "daemon")) || argv.contains(&"--help"));
    let mut command = Process::new(env!("CARGO_BIN_EXE_clipchain-cli"));
    if !argv.contains(&"--db") {
        command.arg("--db").arg(path);
    }
    let mut child = command
        .args(argv)
        .env("XDG_DATA_HOME", path.parent().ok_or("database parent")?)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .ok_or("stdin unavailable")?
        .write_all(input)?;
    Ok(child.wait_with_output()?)
}

fn success(output: &Output) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stderr, b"");
}

fn output_json(output: &Output) -> Result<Value, serde_json::Error> {
    success(output);
    serde_json::from_slice(&output.stdout)
}

fn normalize_help(bytes: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
    let ansi = regex::Regex::new(r"\x1b\[[0-?]*[ -/]*[@-~]")?;
    Ok(ansi
        .replace_all(std::str::from_utf8(bytes)?, "")
        .split_whitespace()
        .map(|token| match token {
            "clipchain-cli.exe" => "clipchain-cli",
            other => other,
        })
        .collect::<Vec<_>>()
        .join(" "))
}

const HELP_FIXTURES: &[(&[&str], &str, &[&str])] = &[
    (&[], "clipchain-cli [OPTIONS] <COMMAND>", &["-V, --version"]),
    (
        &["transform"],
        "clipchain-cli transform [OPTIONS] [TEXT]",
        &["--dry-run"],
    ),
    (&["clip"], "clipchain-cli clip [OPTIONS]", &[]),
    (&["daemon"], "clipchain-cli daemon [OPTIONS]", &[]),
    (&["rules"], "clipchain-cli rules [OPTIONS] <COMMAND>", &[]),
    (
        &["rules", "list"],
        "clipchain-cli rules list [OPTIONS]",
        &["--json"],
    ),
    (
        &["rules", "preview"],
        "clipchain-cli rules preview [OPTIONS] [INPUT]",
        &[],
    ),
    (
        &["rules", "save"],
        "clipchain-cli rules save [OPTIONS] --file <FILE>",
        &["--file <FILE>"],
    ),
    (
        &["rules", "delete"],
        "clipchain-cli rules delete [OPTIONS] <ID>",
        &["--yes"],
    ),
    (
        &["rules", "reorder"],
        "clipchain-cli rules reorder [OPTIONS] <IDS>...",
        &[],
    ),
    (
        &["history"],
        "clipchain-cli history [OPTIONS] <COMMAND>",
        &[],
    ),
    (
        &["history", "list"],
        "clipchain-cli history list [OPTIONS]",
        &["--limit <LIMIT>", "[default: 20]", "--json"],
    ),
    (
        &["history", "clear"],
        "clipchain-cli history clear [OPTIONS]",
        &["--yes"],
    ),
    (
        &["settings"],
        "clipchain-cli settings [OPTIONS] <COMMAND>",
        &[],
    ),
    (
        &["settings", "get"],
        "clipchain-cli settings get [OPTIONS]",
        &["--json"],
    ),
    (
        &["settings", "set"],
        "clipchain-cli settings set [OPTIONS]",
        &[
            "--active <ACTIVE>",
            "--record-history <RECORD_HISTORY>",
            "[possible values: true, false]",
        ],
    ),
    (&["status"], "clipchain-cli status [OPTIONS]", &["--json"]),
];

#[test]
fn real_help_preserves_root_group_and_leaf_usage() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("unopened.db");
    for (argv, usage, options) in HELP_FIXTURES {
        let mut help_argv = argv.to_vec();
        help_argv.push("--help");
        let output = execute(&path, &help_argv, b"")?;
        success(&output);
        let help = normalize_help(&output.stdout)?;
        for expected in [
            format!("Usage: {usage}"),
            "--db <PATH>".into(),
            "-h, --help".into(),
        ] {
            assert!(
                help.contains(&expected),
                "{argv:?}: missing {expected} in {help}"
            );
        }
        for option in *options {
            assert!(
                help.contains(option),
                "{argv:?}: missing {option} in {help}"
            );
        }
        if *argv == ["transform"] || *argv == ["rules", "preview"] {
            for option in [
                "--type <RULE_TYPE>",
                "[possible values: trim, replace, regex]",
                "--target <TARGET>",
                "--pattern <PATTERN>",
                "--replacement <REPLACEMENT>",
                "--ignore-case",
                "--trim-start <TRIM_START>",
                "--trim-end <TRIM_END>",
                "[possible values: true, false]",
            ] {
                assert!(
                    help.contains(option),
                    "{argv:?}: missing {option} in {help}"
                );
            }
        }
    }
    // Names must appear as command entries, not merely somewhere in help prose.
    for (argv, names) in [
        (
            vec![],
            vec![
                "transform",
                "clip",
                "daemon",
                "rules",
                "history",
                "settings",
                "status",
            ],
        ),
        (
            vec!["rules"],
            vec!["list", "preview", "save", "delete", "reorder"],
        ),
        (vec!["history"], vec!["list", "clear"]),
        (vec!["settings"], vec!["get", "set"]),
    ] {
        let mut help_argv = argv;
        help_argv.push("--help");
        let output = execute(&path, &help_argv, b"")?;
        success(&output);
        let help = std::str::from_utf8(&output.stdout)?;
        for name in names {
            assert!(
                help.lines()
                    .any(|line| line.split_whitespace().next() == Some(name)),
                "missing {name}"
            );
        }
    }
    assert!(!path.exists());
    Ok(())
}

#[test]
fn real_help_version_and_error_streams_keep_exit_codes() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("unopened.db");
    for flag in ["-h", "--help", "-V", "--version"] {
        let output = execute(&path, &[flag], b"")?;
        success(&output);
        let text = normalize_help(&output.stdout)?;
        if matches!(flag, "-V" | "--version") {
            assert!(text.starts_with("clipchain-cli "));
            assert_eq!(text.split_whitespace().count(), 2);
        } else {
            assert!(text.contains("Usage: clipchain-cli"));
        }
    }
    for argv in [
        vec!["unknown"],
        vec!["rules", "save"],
        vec!["transform", "a", "b"],
        vec!["history", "list", "--limit", "501"],
        vec!["settings", "set", "--active", "yes"],
    ] {
        let output = execute(&path, &argv, b"")?;
        assert_eq!(output.status.code(), Some(2), "{argv:?}");
        assert_eq!(output.stdout, b"");
        assert!(normalize_help(&output.stderr)?.contains("error:"));
        assert!(serde_json::from_slice::<Value>(&output.stderr).is_err());
    }
    for argv in [
        vec!["history", "clear"],
        vec!["rules", "delete", "1"],
        vec!["settings", "set"],
        vec!["rules", "preview", "text"],
        vec!["transform", "--target", "x"],
        vec!["transform", "--type", "replace", "--replacement", ""],
        vec!["transform", "--type", "replace", "--target", "x"],
        vec!["rules", "preview", "--type", "regex", "--replacement", ""],
        vec!["rules", "preview", "--type", "regex", "--pattern", "x"],
    ] {
        let output = execute(&path, &argv, b"")?;
        assert_eq!(output.status.code(), Some(1), "{argv:?}");
        assert_eq!(output.stdout, b"");
        let error: Value = serde_json::from_slice(&output.stderr)?;
        assert_eq!(error["event"], "error");
        assert_eq!(error["error"]["kind"], "validation");
        assert_ne!(
            error["error"]["message"].as_str().ok_or("error message")?,
            ""
        );
    }
    assert!(!path.exists());
    Ok(())
}

#[test]
fn literal_preview_examples_preserve_stdin_empty_replacements_and_dash_text() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("unopened.db");
    let fixtures: &[(&[&str], &[u8], &str)] = &[
        (
            &["transform", "--type", "trim", "  argument  "],
            b"ignored stdin",
            "argument",
        ),
        (
            &["transform", "--type", "trim"],
            "  é\nline  ".as_bytes(),
            "é\nline",
        ),
        (
            &["transform", "--type", "trim", "--", "-text  "],
            b"",
            "-text",
        ),
        (
            &[
                "rules",
                "preview",
                "--type",
                "trim",
                "--trim-start",
                "false",
                "--trim-end",
                "true",
                "  text  ",
            ],
            b"",
            "  text",
        ),
        (
            &[
                "rules",
                "preview",
                "--type",
                "trim",
                "--trim-start",
                "true",
                "--trim-end",
                "false",
            ],
            b"  text  ",
            "text  ",
        ),
        (
            &[
                "rules",
                "preview",
                "--type",
                "replace",
                "--target",
                "x",
                "--replacement",
                "",
                "--ignore-case",
                "Xx keep",
            ],
            b"",
            " keep",
        ),
        (
            &[
                "rules",
                "preview",
                "--type",
                "regex",
                "--pattern",
                "(\\d+)",
                "--replacement",
                "[$1]",
            ],
            b"item42\n",
            "item[42]\n",
        ),
        (
            &["rules", "preview", "--type", "trim", "--", "-input"],
            b"",
            "-input",
        ),
    ];
    for (argv, input, expected) in fixtures {
        let output = execute(&path, argv, input)?;
        success(&output);
        assert_eq!(output.stdout, expected.as_bytes(), "{argv:?}");
    }
    let report = output_json(&execute(
        &path,
        &["transform", "--type", "trim", "--dry-run", " x "],
        b"",
    )?)?;
    assert_eq!(
        report,
        json!({"changed": true, "inputBytes": 3, "outputBytes": 1, "activeRuleCount": 1, "transformedText": "x"})
    );
    assert!(!path.exists());
    Ok(())
}

#[tokio::test]
async fn saved_information_is_available_with_no_desktop_or_worker() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("saved data.db");
    let pool = db::open(&path).await?;
    let rule: Rule = serde_json::from_value(
        json!({"type": "trim", "data": {"id": 0, "name": "Offline rule", "order": 0, "isActive": true, "trimStart": true, "trimEnd": true}}),
    )?;
    db::rules::save(&pool, rule).await?;
    db::settings::patch_monitor(&pool, Some(false), Some(false)).await?;
    for index in 0..21 {
        db::history::insert(&pool, &format!("before {index}"), &format!("after {index}")).await?;
    }
    let expected_rules = serde_json::to_value(db::rules::get_all(&pool).await?)?;
    let expected_settings = serde_json::to_value(db::settings::get(&pool).await?)?;
    let expected_history = serde_json::to_value(db::history::get_all(&pool).await?)?;
    pool.close().await;

    // --db after nested leaves must work in the executable too, including spaces.
    let database = path.to_str().ok_or("database path")?;
    assert_eq!(
        output_json(&execute(
            &path,
            &["rules", "list", "--json", "--db", database],
            b""
        )?)?,
        expected_rules
    );
    assert_eq!(
        output_json(&execute(
            &path,
            &["settings", "get", "--db", database, "--json"],
            b""
        )?)?,
        expected_settings
    );
    let history = output_json(&execute(&path, &["history", "list", "--json"], b"")?)?;
    assert_eq!(history.as_array().ok_or("history array")?.len(), 20);
    assert_eq!(history[0]["originalText"], "before 20");
    assert_eq!(history[19]["originalText"], "before 1");
    assert_eq!(
        output_json(&execute(
            &path,
            &[
                "history", "list", "--limit", "500", "--json", "--db", database
            ],
            b""
        )?)?,
        expected_history
    );
    assert_eq!(
        output_json(&execute(
            &path,
            &["history", "list", "--limit", "1", "--json"],
            b""
        )?)?,
        json!([expected_history[0]])
    );

    let output = execute(&path, &["rules", "list"], b"")?;
    success(&output);
    assert!(std::str::from_utf8(&output.stdout)?.contains("Offline rule"));
    let output = execute(&path, &["settings", "get"], b"")?;
    success(&output);
    assert!(std::str::from_utf8(&output.stdout)?.contains("active: false"));
    let output = execute(&path, &["history", "list", "--limit", "1"], b"")?;
    success(&output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout)?,
        expected_history[0]
    );

    // Each read ran in its own process after the fixture connection was closed.
    // Inspecting saved data must preserve rules, settings and retained history.
    let pool = db::open(&path).await?;
    assert_eq!(
        serde_json::to_value(db::rules::get_all(&pool).await?)?,
        expected_rules
    );
    assert_eq!(
        serde_json::to_value(db::settings::get(&pool).await?)?,
        expected_settings
    );
    assert_eq!(
        serde_json::to_value(db::history::get_all(&pool).await?)?,
        expected_history
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn literal_management_examples_and_confirmation_preserve_the_contract() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("test.db");
    let file = directory.path().join("rule file.json");
    for name in ["First", "Second", "Third"] {
        std::fs::write(
            &file,
            serde_json::to_vec(
                &json!({"type": "trim", "data": {"id": 0, "name": name, "order": 0, "isActive": true, "trimStart": true, "trimEnd": true}}),
            )?,
        )?;
        let saved = output_json(&execute(
            &path,
            &["rules", "save", "--file", file.to_str().ok_or("rule path")?],
            b"",
        )?)?;
        assert_eq!(saved["data"]["name"], name);
    }
    let reordered = output_json(&execute(&path, &["rules", "reorder", "3", "1", "2"], b"")?)?;
    assert_eq!(
        reordered
            .as_array()
            .ok_or("rules array")?
            .iter()
            .map(|rule| rule["data"]["id"].clone())
            .collect::<Vec<_>>(),
        vec![json!(3), json!(1), json!(2)]
    );
    let settings = output_json(&execute(
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
    )?)?;
    assert_eq!(settings["isActive"], false);
    assert_eq!(settings["recordHistory"], false);
    let settings = output_json(&execute(
        &path,
        &["settings", "set", "--active", "true"],
        b"",
    )?)?;
    assert_eq!(settings["isActive"], true);
    assert_eq!(settings["recordHistory"], false);
    let pool = db::open(&path).await?;
    db::history::insert(&pool, "before", "after").await?;
    pool.close().await;
    for argv in [vec!["rules", "delete", "1"], vec!["history", "clear"]] {
        let output = execute(&path, &argv, b"")?;
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(output.stdout, b"");
        let error: Value = serde_json::from_slice(&output.stderr)?;
        assert_eq!(error["error"]["kind"], "validation");
    }
    assert_eq!(
        output_json(&execute(&path, &["rules", "list", "--json"], b"")?)?,
        reordered
    );
    assert_eq!(
        output_json(&execute(&path, &["history", "list", "--json"], b"")?)?
            .as_array()
            .ok_or("history array")?
            .len(),
        1
    );
    assert_eq!(
        output_json(&execute(&path, &["rules", "delete", "1", "--yes"], b"")?)?,
        json!({"deleted": 1})
    );
    assert_eq!(
        output_json(&execute(&path, &["history", "clear", "--yes"], b"")?)?,
        json!({"cleared": true})
    );
    let rules = output_json(&execute(&path, &["rules", "list", "--json"], b"")?)?;
    assert_eq!(rules.as_array().ok_or("rules array")?.len(), 2);
    assert_eq!(rules[0]["data"]["id"], 3);
    assert_eq!(rules[1]["data"]["id"], 2);
    assert_eq!(
        output_json(&execute(&path, &["history", "list", "--json"], b"")?)?,
        json!([])
    );
    Ok(())
}
