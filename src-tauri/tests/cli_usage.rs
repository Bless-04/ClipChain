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
    assert!(
        parsed.db.is_none(),
        "Omitting --db must leave database selection to the shared default resolver. ({argv:?})"
    );
    assert_eq!(
        operation(parsed.command),
        *expected,
        "The published command spelling must select the expected operation and argument values. ({argv:?})"
    );
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
            Some(Path::new("custom data/clipchain.db")),
            "The global --db option must preserve custom paths, including spaces."
        );
        assert_eq!(
            operation(parsed.command),
            *expected,
            "Moving --db must not change the selected command or any of its arguments. ({argv:?} at {index})"
        );
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
            assert_eq!(
                rule["type"], options[1],
                "Both draft entry points must select the rule type requested by --type."
            );
            for (key, value) in expected.as_object().ok_or("expected draft fields")? {
                assert_eq!(
                    rule["data"][key], *value,
                    "Draft options must preserve explicit values and documented defaults. ({prefix:?} {options:?}: {key})"
                );
            }
        }
    }
    let parsed = Cli::try_parse_from(["clipchain-cli", "transform"])?;
    let Command::Transform { draft, .. } = parsed.command else {
        return Err("wrong command".into());
    };
    assert!(
        draft.rule()?.is_none(),
        "A transform without --type must use saved rules rather than inventing a draft."
    );
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
        assert_eq!(
            error.exit_code(),
            2,
            "Malformed arguments must use parser exit code 2. ({argv:?})"
        );
        assert!(
            error.use_stderr(),
            "Argument errors must be written to stderr rather than mixed with command results. ({argv:?})"
        );
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
        assert_eq!(
            error.kind(),
            kind,
            "Help and version aliases must select the requested informational action: {flag}."
        );
        assert_eq!(
            error.exit_code(),
            0,
            "Requesting help or version is successful and must exit with code 0."
        );
        assert!(
            !error.use_stderr(),
            "Help and version must be printed on stdout, not stderr."
        );
    }
}

fn execute(path: &Path, argv: &[&str], input: &[u8]) -> Result<Output, Box<dyn std::error::Error>> {
    // A test edit cannot accidentally start a native worker through this helper.
    assert!(
        !argv.iter().any(|arg| matches!(*arg, "clip" | "daemon")) || argv.contains(&"--help"),
        "Usage tests must never execute real clipboard processing; clip and daemon are allowed only with --help."
    );
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
        "The example command must succeed with exit code 0; captured stderr follows. ({})",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stderr, b"",
        "Successful usage examples must not print errors or diagnostics on stderr."
    );
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
                "Help must retain the published usage form and global options. ({argv:?}: missing {expected} in {help})"
            );
        }
        for option in *options {
            assert!(
                help.contains(option),
                "Help must advertise the documented option spelling and value placeholder. ({argv:?}: missing {option} in {help})"
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
                    "Both draft entry points must advertise all shared draft options and accepted values. ({argv:?}: missing {option} in {help})"
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
                "Group help must list each published subcommand as a command entry. (missing {name})"
            );
        }
    }
    assert!(
        !path.exists(),
        "Requesting help must not create a database."
    );
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
            assert!(
                text.starts_with("clipchain-cli "),
                "Version output must identify the console executable as clipchain-cli."
            );
            assert_eq!(
                text.split_whitespace().count(),
                2,
                "Version output must contain only the program name and version, without extra prose."
            );
        } else {
            assert!(
                text.contains("Usage: clipchain-cli"),
                "Both help aliases must show the root command usage."
            );
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
        assert_eq!(
            output.status.code(),
            Some(2),
            "Parser errors must exit with code 2 so scripts can distinguish invalid syntax. ({argv:?})"
        );
        assert_eq!(
            output.stdout, b"",
            "A parser error must not emit result data on stdout."
        );
        assert!(
            normalize_help(&output.stderr)?.contains("error:"),
            "A parser failure must include a human-readable error on stderr."
        );
        assert!(
            serde_json::from_slice::<Value>(&output.stderr).is_err(),
            "Parser failures must remain human-readable usage errors rather than application JSON errors."
        );
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
        assert_eq!(
            output.status.code(),
            Some(1),
            "Application validation failures must use exit code 1. ({argv:?})"
        );
        assert_eq!(
            output.stdout, b"",
            "Application validation failures must leave stdout empty."
        );
        let error: Value = serde_json::from_slice(&output.stderr)?;
        assert_eq!(
            error["event"], "error",
            "Structured application failures must identify themselves as error events."
        );
        assert_eq!(
            error["error"]["kind"], "validation",
            "Missing confirmations or invalid drafts must be classified as validation failures."
        );
        assert_ne!(
            error["error"]["message"].as_str().ok_or("error message")?,
            "",
            "A typed error must include an explanation a person can act on."
        );
    }
    assert!(
        !path.exists(),
        "Help, version, and rejected input must not create the selected database."
    );
    Ok(())
}

#[test]
// Literal argv fixtures and their failure explanations stay together so the
// published examples remain easy to compare with their expected results.
#[allow(clippy::too_many_lines)]
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
        assert_eq!(
            output.stdout,
            expected.as_bytes(),
            "The literal usage example must preserve the promised text, whitespace, and newline behavior. ({argv:?})"
        );
    }
    let report = output_json(&execute(
        &path,
        &["transform", "--type", "trim", "--dry-run", " x "],
        b"",
    )?)?;
    assert_eq!(
        report,
        json!({"changed": true, "inputBytes": 3, "outputBytes": 1, "activeRuleCount": 1, "transformedText": "x"}),
        "A dry run must report byte counts, active rules, whether text changed, and the final text."
    );
    assert!(!path.exists(), "Draft examples must not create a database.");
    Ok(())
}

#[tokio::test]
// Keep setup, offline reads, and unchanged-data checks in one readable scenario;
// explanatory messages account for the extra lines.
#[allow(clippy::too_many_lines)]
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
        expected_rules,
        "Rules must be readable from a closed database without launching the desktop app or worker."
    );
    assert_eq!(
        output_json(&execute(
            &path,
            &["settings", "get", "--db", database, "--json"],
            b""
        )?)?,
        expected_settings,
        "Settings must be readable with --db after the nested command and no running desktop app."
    );
    let history = output_json(&execute(&path, &["history", "list", "--json"], b"")?)?;
    assert_eq!(
        history.as_array().ok_or("history array")?.len(),
        20,
        "History listing must default to the newest 20 entries."
    );
    assert_eq!(
        history[0]["originalText"], "before 20",
        "The first history entry must be the newest recorded copy."
    );
    assert_eq!(
        history[19]["originalText"], "before 1",
        "The default limit must exclude the oldest of the 21 fixture entries."
    );
    assert_eq!(
        output_json(&execute(
            &path,
            &[
                "history", "list", "--limit", "500", "--json", "--db", database
            ],
            b""
        )?)?,
        expected_history,
        "An explicit larger limit must return all retained fixture history in order."
    );
    assert_eq!(
        output_json(&execute(
            &path,
            &["history", "list", "--limit", "1", "--json"],
            b""
        )?)?,
        json!([expected_history[0]]),
        "A history limit of one must return only the newest entry."
    );

    let output = execute(&path, &["rules", "list"], b"")?;
    success(&output);
    assert!(
        std::str::from_utf8(&output.stdout)?.contains("Offline rule"),
        "Human-readable rule output must include the saved rule name."
    );
    let output = execute(&path, &["settings", "get"], b"")?;
    success(&output);
    assert!(
        std::str::from_utf8(&output.stdout)?.contains("active: false"),
        "Human-readable settings must show the saved paused state."
    );
    let output = execute(&path, &["history", "list", "--limit", "1"], b"")?;
    success(&output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout)?,
        expected_history[0],
        "Human-readable history must retain the complete newest entry as one JSON object."
    );

    // Each read ran in its own process after the fixture connection was closed.
    // Inspecting saved data must preserve rules, settings and retained history.
    let pool = db::open(&path).await?;
    assert_eq!(
        serde_json::to_value(db::rules::get_all(&pool).await?)?,
        expected_rules,
        "Extracting information must leave saved rules unchanged."
    );
    assert_eq!(
        serde_json::to_value(db::settings::get(&pool).await?)?,
        expected_settings,
        "Extracting information must leave saved settings unchanged."
    );
    assert_eq!(
        serde_json::to_value(db::history::get_all(&pool).await?)?,
        expected_history,
        "Extracting information must leave retained clipboard history unchanged."
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
// These commands depend on the state produced by earlier steps. Keep the full
// scenario and its descriptive assertions together rather than hiding the flow.
#[allow(clippy::too_many_lines)]
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
        assert_eq!(
            saved["data"]["name"], name,
            "Saving a rule file must return its persisted name."
        );
    }
    let reordered = output_json(&execute(&path, &["rules", "reorder", "3", "1", "2"], b"")?)?;
    assert_eq!(
        reordered
            .as_array()
            .ok_or("rules array")?
            .iter()
            .map(|rule| rule["data"]["id"].clone())
            .collect::<Vec<_>>(),
        vec![json!(3), json!(1), json!(2)],
        "Reorder must preserve the exact sequence of IDs supplied by the user."
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
    assert_eq!(
        settings["isActive"], false,
        "An explicit --active false must pause automatic monitoring."
    );
    assert_eq!(
        settings["recordHistory"], false,
        "An explicit --record-history false must disable recording."
    );
    let settings = output_json(&execute(
        &path,
        &["settings", "set", "--active", "true"],
        b"",
    )?)?;
    assert_eq!(
        settings["isActive"], true,
        "An explicit --active true must resume automatic monitoring."
    );
    assert_eq!(
        settings["recordHistory"], false,
        "Changing active state alone must preserve disabled recording."
    );
    let pool = db::open(&path).await?;
    db::history::insert(&pool, "before", "after").await?;
    pool.close().await;
    for argv in [vec!["rules", "delete", "1"], vec!["history", "clear"]] {
        let output = execute(&path, &argv, b"")?;
        assert_eq!(
            output.status.code(),
            Some(1),
            "Destructive commands without --yes must fail with validation exit code 1."
        );
        assert_eq!(
            output.stdout, b"",
            "An unconfirmed destructive command must not print a success result."
        );
        let error: Value = serde_json::from_slice(&output.stderr)?;
        assert_eq!(
            error["error"]["kind"], "validation",
            "Missing destructive confirmation must produce a typed validation error."
        );
    }
    assert_eq!(
        output_json(&execute(&path, &["rules", "list", "--json"], b"")?)?,
        reordered,
        "An unconfirmed deletion must preserve all rules and their order."
    );
    assert_eq!(
        output_json(&execute(&path, &["history", "list", "--json"], b"")?)?
            .as_array()
            .ok_or("history array")?
            .len(),
        1,
        "An unconfirmed clear must preserve the retained history entry."
    );
    assert_eq!(
        output_json(&execute(&path, &["rules", "delete", "1", "--yes"], b"")?)?,
        json!({"deleted": 1}),
        "A confirmed deletion must report the deleted rule ID."
    );
    assert_eq!(
        output_json(&execute(&path, &["history", "clear", "--yes"], b"")?)?,
        json!({"cleared": true}),
        "A confirmed history clear must report success."
    );
    let rules = output_json(&execute(&path, &["rules", "list", "--json"], b"")?)?;
    assert_eq!(
        rules.as_array().ok_or("rules array")?.len(),
        2,
        "Deleting one of three rules must leave the other two intact."
    );
    assert_eq!(
        rules[0]["data"]["id"], 3,
        "Deleting a rule must preserve the relative order of the remaining rules."
    );
    assert_eq!(
        rules[1]["data"]["id"], 2,
        "The final remaining rule must retain its original ID after deletion."
    );
    assert_eq!(
        output_json(&execute(&path, &["history", "list", "--json"], b"")?)?,
        json!([]),
        "Confirmed clearing must leave no retained history entries."
    );
    Ok(())
}
