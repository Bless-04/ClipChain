# ClipChain CLI

`clipchain-cli` is a console executable that uses the same Rust pipeline and
SQLite repositories as the desktop app. It does not construct a Tauri app,
window, or webview. Building it currently requires the Rust and Tauri system
prerequisites described in [README.md](README.md).

Build from the repository root:

```powershell
cargo build --locked --manifest-path src-tauri/Cargo.toml --bin clipchain-cli
./src-tauri/target/debug/clipchain-cli.exe --help
```

On macOS/Linux, omit `.exe`. Release builds use `--release` and write to
`src-tauri/target/release/`.

## Extract saved app information

The desktop app can be closed. These commands open its saved SQLite data
directly and print it to the terminal; they do not start a window, webview,
clipboard monitor, or daemon:

```powershell
./src-tauri/target/debug/clipchain-cli.exe rules list --json
./src-tauri/target/debug/clipchain-cli.exe history list --limit 500 --json
./src-tauri/target/debug/clipchain-cli.exe settings get --json
./src-tauri/target/debug/clipchain-cli.exe status --json
```

Use `--json` for scripts, or redirect stdout to a file to export a result.
`rules list` includes all saved rule definitions in pipeline order; `history
list` includes retained original and transformed text, newest first; `settings
get` includes the saved monitoring, recording, and startup preferences. `status`
adds the selected database path, rule counts, and any current worker's health.
With the app and daemon stopped, the worker has no owner; saved data is still
available. Startup preference is not a readback of OS registration.

To inspect another database, add `--db PATH` before the command or after a nested
subcommand, for example `rules list --json --db ./saved/clipchain.db`.
Inspection preserves saved rules, settings, and history, but database opening
still creates missing files and applies migrations as described below. A fresh
database contains defaults and empty lists, not data extracted from another app
instance. Run `clip` or `daemon` only when clipboard processing is desired.

## Database selection

Every command accepts a global `--db PATH`, including after the subcommand.
Without an override, the GUI and CLI both resolve
`dirs::data_dir()/com.blessing.clipchain/clipchain.db`, normally
`%APPDATA%\com.blessing.clipchain\clipchain.db` on Windows. Database commands
create missing parent directories and apply embedded migrations. An override
selects a different database; it does not create a separate OS clipboard.

Draft previews and transforms with `--type` do not open or create a database.

## Transform text

```powershell
clipchain-cli transform '  copy  '
Get-Content -Raw input.txt | clipchain-cli transform
clipchain-cli --db ./scratch.db transform '  text  ' --dry-run
clipchain-cli transform '  text  ' --type trim
clipchain-cli transform 'secret' --type replace --target secret --replacement '[REDACTED]'
```

The default transform evaluates saved active rules in ascending pipeline order.
An ad-hoc `--type` evaluates a single draft instead of the saved pipeline.
Neither mode writes clipboard contents, rules, or history. `--dry-run` emits a
JSON report with `changed`, `inputBytes`, `outputBytes`, `activeRuleCount`, and
`transformedText`.

Without positional text, the command reads one UTF-8 payload from stdin through
EOF, retaining embedded line breaks and whitespace. It does not process each
line separately. Input, intermediate outputs, and final output are limited to
1 MiB in UTF-8 bytes. Reads stop after the limit plus one byte. An empty payload
is valid. Standard output contains only the result, with no added newline.
Use `--` before positional text beginning with `-`. Shells can add a trailing
newline or convert encodings when piping; the CLI processes the bytes received.

## Preview and manage rules

```powershell
clipchain-cli rules list
clipchain-cli rules list --json
clipchain-cli rules preview --type trim --trim-start false '  text  '
clipchain-cli rules preview --type replace --target SECRET --replacement '' --ignore-case 'secret text'
clipchain-cli rules preview --type regex --pattern '(\d+)' --replacement '[$1]' 'item42'
clipchain-cli rules save --file ./rule.json
clipchain-cli rules reorder 3 1 2
clipchain-cli rules delete 3 --yes
```

Trim defaults to trimming both ends; `--trim-start true|false` and
`--trim-end true|false` select sides. Replace requires `--target` and
`--replacement`; `--ignore-case` selects the core's Unicode lowercase matching.
Regex requires `--pattern` and `--replacement`, supports Rust capture references,
and uses inline flags such as `(?i)` for case-insensitive matching. Empty
replacements remove matches. Invalid or irrelevant draft flags are rejected.

`rules save` accepts one tagged rule object, matching an element of
`rules list --json`. A new rule has `id: 0`; existing IDs edit the corresponding
rule without changing its position. New rules append. Editing `isActive` enables
or disables a rule. For example:

```json
{
  "type": "trim",
  "data": {
    "id": 0,
    "name": "Trim whitespace",
    "order": 0,
    "isActive": true,
    "trimStart": true,
    "trimEnd": true
  }
}
```

Rule files are limited to 128 KiB, and normal core rule/field limits still apply.
Reordering requires every saved ID exactly once; a failed reorder rolls back.
Deletion requires `--yes`. Changes persist immediately and the worker reads
saved rules on subsequent polls. Reopen/reload the desktop UI to refresh views
after external edits; CLI mutations do not emit desktop IPC events.

## Clipboard processing

```powershell
clipchain-cli clip
clipchain-cli daemon
```

`clip` transforms the existing text once, even if automatic monitoring is paused.
It skips non-text contents and unchanged results, re-reads before writing to
avoid replacing a newer copy, and records history only after a successful write
when recording is enabled. It returns JSON with `changed` and `historyRecorded`.
A failed history save reports that the clipboard was already transformed.

`daemon` runs the shared 250 ms monitor until Ctrl+C, or SIGTERM on Unix. Startup
and resume establish a baseline without transforming the current clipboard.
Own writes are suppressed, transient read/write/database failures are retryable,
and shutdown joins the worker before releasing ownership. Logs are JSON Lines
on stderr with `status`, `transformation`, or `error` events. Transformation logs
include only recording metadata, never clipboard text.

The GUI, daemon, and one-shot command acquire the same OS file lock in the
default application data directory. Close the desktop app before starting a
daemon or one-shot command. A paused watcher retains ownership. This lock spans
database overrides and is released automatically if the process exits or fails;
do not delete the lock file while a process may own it.

On Linux, clipboard ownership can require a resident process. One-shot text may
become unavailable when the CLI exits if the desktop clipboard manager has not
retained it. Use the daemon for resident ownership; see the
[arboard platform behavior](https://docs.rs/arboard/3.6.1/arboard/struct.Clipboard.html#linux).
Clipboard commands require an accessible desktop clipboard session.

## History, settings, and status

```powershell
clipchain-cli history list --limit 20 --json
clipchain-cli history clear --yes
clipchain-cli settings get --json
clipchain-cli settings set --active false
clipchain-cli settings set --active true --record-history false
clipchain-cli status --json
```

History lists newest first. `--limit` accepts 1–500 and defaults to 20. Text mode
also uses one escaped JSON object per entry to preserve multiline text safely.
Clearing requires `--yes` and preserves rules/settings. History output explicitly
includes retained clipboard text.

Settings patches are transactional and preserve fields not requested. A durable
pause marker resets the worker baseline even when a peer pauses and resumes
between two polls. Startup registration is managed by the desktop UI; the CLI
reports its stored preference without claiming OS readback or changing it.

Status includes the selected database path, total and active rule counts,
settings, and the global clipboard owner's database/health. No owner means
stopped; an owner with `status: null` means health could not be read. A live
owner can be using another database. Stale health files are ignored after the
OS lock is released.

Successful commands exit with code 0. Operational/validation errors exit with
code 1 and a typed JSON error on stderr. Argument parser errors use code 2 and
Clap's human-readable help; `--help` and `--version` exit successfully.
