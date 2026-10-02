# ClipChain

A local desktop clipboard transformation pipeline built with Tauri, SvelteKit, and Rust.

## Recommended IDE Setup

[VS Code](https://code.visualstudio.com/) + [Svelte](https://marketplace.visualstudio.com/items?itemName=svelte.svelte-vscode) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer).

## Develop

Use Node.js, pnpm **11.25.0** (pinned in `package.json`), and a Rust toolchain.
Windows builds also need the MSVC C++ build tools, Windows SDK, and WebView2.
Run commands from this repository root in PowerShell:

```powershell
pnpm install --frozen-lockfile
pnpm tauri dev
```

`pnpm dev` opens browser development at `http://localhost:1420`. Browser mode
supports navigation and draft editing, and explicitly disables native saves,
previews, clipboard monitoring, and settings. It does not simulate persistence.
Close ClipChain to stop its monitor; no tray/background-on-close feature exists.

The local `pnpm-workspace.yaml` prevents a parent workspace from redirecting
installs. Use `pnpm add` / `pnpm remove` for dependency changes and keep the
pnpm-generated lockfile. Do not add an npm lockfile. Only the locked esbuild
version is allowed to run its install script.

After moving or renaming the repository, stale Cargo build metadata can refer
to the old directory. `cargo clean --manifest-path src-tauri/Cargo.toml -p tauri`
refreshes the affected generated permission paths. Reinstall locked frontend
dependencies if their store/module paths need refreshing.

Migration SQL uses LF line endings, enforced by `.gitattributes`, because SQLx
checksums the exact file bytes. Keep applied migrations unchanged and add a new
numbered migration for schema changes. A line-ending mismatch is not a reason
to delete the database or rewrite its migration metadata.

## Workflow

- **Overview:** actual worker health, pause/resume, active rule count, and recent
  retained transformations. History counts are not lifetime transformation totals.
- **Rules:** create, edit, enable/disable, delete with confirmation, and reorder
  with keyboard-accessible arrows. Each step receives the previous step's output.
- **Preview:** runs one draft through Rust without saving or touching the
  clipboard, including disabled drafts. An empty replacement removes matches.
- **Regex:** edit an arbitrary Rust regex or explicitly apply the visual builder.
  Builder text fields are literal and escaped. Captures use `$1` or `${name}`;
  `${1}text` separates a numeric capture from following text, and `$$` inserts `$`.
- **History:** newest first, 20 entries per page. Expanding an entry preserves
  whitespace and highlights a contiguous changed span. Long values show bounded
  excerpts; several edits can fall within the same highlighted span.
- **Settings:** monitor, native startup registration, and history recording.
  Controls show saved values only after the backend confirms each change.

## Storage and privacy

Rust resolves the database through Tauri's `app_data_dir()`. The application
identifier is `com.blessing.clipchain` and the database filename is
`clipchain.db`. On Windows the normal data location is:

```text
%APPDATA%\com.blessing.clipchain\clipchain.db
```

Browser preference keys use the `clipchain:` namespace. The rename was completed
before any user data existed, so no data migration is needed.

Rules, settings, and up to **500** history entries are stored in ordinary local
SQLite with WAL and embedded migrations. Recorded history contains unencrypted
clipboard text. Disabling recording stops future retention while transformations
continue; it does not remove existing history. Clear history removes retained
entries without changing rules/settings and does not guarantee secure erasure.
Clipboard contents are not logged or transmitted by the application.

## Behavior and limits

The monitor polls text every 250 ms. Startup and resume establish a baseline;
they do not transform the text already present. Unchanged text, non-text content,
and no-op transformations do not create history. The worker suppresses its own
writes and re-reads before writing to reduce the risk of overwriting a newer copy.
That check is best-effort, not an atomic OS operation. Transient rule-load and clipboard-read failures keep pending text retryable. Failed writes can retry;
history is recorded only after successful writes.

Content polling cannot distinguish a second copy of identical text or guarantee
capture of copies shorter than one polling interval. Limits: 100 rules, 120
Unicode characters per rule name, 16,384 UTF-8 bytes per rule field, and 1 MiB for
input and transformed output. Case-insensitive literal matching uses Unicode
lowercase sequences, not locale-specific or complete Unicode case folding.
Rust regex syntax is authoritative; JavaScript regex compatibility is not assumed.
Text replacement/redaction is not a general defense against prompt injection.

## Architecture and checks

Rust owns transformations, validation, clipboard access, startup registration,
and persistent data. SQL stays in `src-tauri/src/db`. The shared `ipc_builder()`
registry generates the commands, events, domain types, and limits used by the
frontend. Regenerate after changing any Rust IPC contract:

```powershell
pnpm codegen
pnpm check
pnpm test
pnpm lint
pnpm build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
git diff --check
```

Services alone access native APIs. Per-layout class-based rune state keeps
confirmed data, ignores superseded reads, coalesces events during mutations,
and cleans up async listener registration on teardown. Generated bindings are
owned by tauri-specta; do not edit them by hand. Their unused `Channel` import
currently produces a non-fatal build warning from the pinned generator.

```powershell
pnpm tauri build --debug --no-bundle
```

This builds a native debug executable in `src-tauri/target/debug/clipchain.exe`
without creating an installer. Frontend bundles, native executables, and release
packaging are separate verification steps.

See `HANDOFF_PROMPT.md` for fresh test results and remaining native acceptance
checks. Regular test suites use temporary databases and simulated clipboard
devices. Live native acceptance uses a disposable app identity and synthetic text,
with the original clipboard restored afterward.

Run `pnpm test:all` for both frontend and Rust regression suites (`pnpm test`
and `pnpm test:rust` run them separately). `pnpm ci:quick` runs the lightweight
push checks: frontend tests, types, lint, and Rust formatting. Pull requests and
manual dispatch run the full Windows, macOS, and Linux matrix, including both
test suites, native builds, and real clipboard smoke tests on disposable hosted
runners. See [TESTING.md](TESTING.md) for the coverage map, CI triggers, native
acceptance boundaries, and expectations for new features.
