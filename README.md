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

## Project Structure

```text
ClipChain/
├── .github/workflows/          # CI pipelines
│   ├── CI.yml                  # Quick push checks (types, lint, Vitest, Rust formatting)
│   └── cross-platform.yml      # Full matrix (Ubuntu, Windows, macOS) with native smoke tests
├── src/                        # SvelteKit frontend (TypeScript, Svelte 5 runes)
│   ├── app.css                 # Global CSS design tokens and theme styles
│   ├── app.html                # HTML entry document
│   ├── lib/
│   │   ├── components/         # UI components & panels
│   │   │   ├── rules/          # Rule list, card, modal, and editors (Trim, Replace, Regex)
│   │   │   │   ├── RegexRuleEditor.svelte
│   │   │   │   ├── ReplaceRuleEditor.svelte
│   │   │   │   ├── RuleCard.svelte
│   │   │   │   ├── RuleEditor.svelte
│   │   │   │   ├── RuleList.svelte
│   │   │   │   └── TrimRuleEditor.svelte
│   │   │   ├── ui/             # Reusable primitives (Badge, Button, Input, LoadError, Modal, Switch)
│   │   │   ├── HistoryItem.svelte    # History entry row with contiguous diff highlighting
│   │   │   ├── HistoryPanel.svelte   # Paginated history view and clear confirmation
│   │   │   └── SettingsPanel.svelte  # Monitor toggle, startup auto-launch, history recording
│   │   ├── constants/          # Application constants (routes, events, rules, storage keys, UI limits)
│   │   ├── generated/          # Auto-generated TypeScript bindings (tauri-specta)
│   │   │   └── bindings.ts     # Pinned Specta types, command wrappers, and event names
│   │   ├── history/            # Text diff utilities
│   │   │   └── diff.ts         # Contiguous changed span computation for history items
│   │   ├── rules/              # Rule draft helper models and validation
│   │   │   └── draft.ts        # Draft rule factory, mutation helpers, and validation
│   │   ├── services/           # Typed IPC services with browser mock fallbacks
│   │   │   ├── desktop.service.ts    # Tauri core runtime detection and event listener setup
│   │   │   ├── history.service.ts    # History retrieval and clearing
│   │   │   ├── result.ts             # Typed AppResult error handling
│   │   │   ├── rules.service.ts      # Rule retrieval, mutation, ordering, and preview
│   │   │   └── settings.service.ts   # Settings persistence and monitor control
│   │   └── state/              # Class-based reactive state singletons (Svelte 5 runes)
│   │       ├── app.svelte.ts         # Central lifecycle and connection coordinator
│   │       ├── history.svelte.ts     # Paginated history state and cache invalidation
│   │       ├── resource.svelte.ts    # Generic async resource rune with caching/revalidation
│   │       ├── rules.svelte.ts       # Reactive rules cache and optimistic mutations
│   │       └── settings.svelte.ts    # Settings synchronization and monitor state
│   ├── routes/                 # SvelteKit page routes (file-based routing)
│   │   ├── +layout.svelte      # Main application shell, sidebar navigation, status indicator
│   │   ├── +layout.ts          # Static adapter prerender configuration
│   │   ├── +page.svelte        # Overview / Dashboard: worker health, quick stats, recent items
│   │   ├── history/+page.svelte# Bounded transformation history browser
│   │   ├── rules/+page.svelte  # Pipeline rule configuration and reordering
│   │   └── settings/+page.svelte# Monitor toggles and application preferences
│   └── test/                   # Frontend unit tests, mocks (desktop-mock.ts, kit-mock.ts), fixtures
├── src-tauri/                  # Tauri v2 desktop application & Rust backend
│   ├── bin/
│   │   ├── cli/                # CLI argument and subcommand specifications
│   │   │   └── args.rs
│   │   ├── cli.rs              # Headless CLI entry point (clipchain-cli)
│   │   └── generate_bindings.rs # Standalone binary to regenerate TypeScript Specta bindings
│   ├── capabilities/           # Tauri v2 security capabilities (default.json)
│   ├── icons/                  # Application icons across target platforms
│   ├── src/
│   │   ├── clipboard/          # 250 ms clipboard poller, pre-write guard, own-write suppression
│   │   │   ├── mod.rs          # Background monitor loop and event dispatcher
│   │   │   ├── native_tests.rs # Real OS clipboard smoke tests for CI
│   │   │   ├── ownership.rs    # Inter-process single-consumer clipboard lease and worker health
│   │   │   └── tests.rs        # Isolated monitor tests with mock clipboard adapter
│   │   ├── commands/           # Tauri IPC command handlers
│   │   │   ├── mod.rs          # Command module exports
│   │   │   ├── history.rs      # get_history, clear_history
│   │   │   ├── rules.rs        # get_rules, save_rule, delete_rule, reorder_rules, preview_rule
│   │   │   └── settings.rs     # get_settings, save_settings, get_monitor_status
│   │   ├── db/                 # SQLite storage layer with WAL mode
│   │   │   ├── migrations/     # Embedded SQLx migration scripts (LF line endings)
│   │   │   │   ├── 0001_initial.sql         # Initial schema for rules, settings, and history
│   │   │   │   └── 0002_monitor_control.sql # Inter-process pause generation and monitor coordination
│   │   │   ├── history.rs      # Bounded history insertion (FIFO cap: 500) and retrieval
│   │   │   ├── mod.rs          # Database initialization and connection pool
│   │   │   ├── rules.rs        # Rule CRUD and transaction-safe position reordering
│   │   │   ├── settings.rs     # Key-value and typed settings persistence
│   │   │   └── tests.rs        # Database integration and persistence tests
│   │   ├── models/             # Shared Rust domain models
│   │   │   ├── events.rs       # Specta-exported IPC event payload definitions
│   │   │   ├── history.rs      # HistoryEntry and HistoryList types
│   │   │   ├── mod.rs          # Model module exports
│   │   │   ├── rule.rs         # Rule, RuleDraft, and RuleType definitions
│   │   │   └── settings.rs     # AppSettings and MonitorStatus types
│   │   ├── pipeline/           # Clipboard text transformation engine
│   │   │   ├── mod.rs          # Pipeline module exports
│   │   │   ├── service.rs      # Sequential pipeline runner chaining active rules
│   │   │   ├── transformer.rs  # Transform implementations (Trim, literal Replace, Rust Regex)
│   │   │   └── validation.rs   # Input boundary and regex validation
│   │   ├── constants.rs        # Shared limits (100 rules, 1 MiB text, 250 ms poll interval)
│   │   ├── error.rs            # Typed AppError enumeration with Specta serialization
│   │   ├── lib.rs              # Library root, ipc_builder() registry, plugin setup, lifecycle
│   │   ├── main.rs             # Application entry point with Windows subsystem configuration
│   │   └── state.rs            # AppState holding the SQLite connection pool and shutdown flag
│   ├── tests/                  # Integration test suites
│   │   └── cli.rs              # Headless CLI end-to-end integration tests
│   ├── Cargo.lock              # Pinned Rust dependencies lockfile
│   ├── Cargo.toml              # Rust crate manifest, dependencies, and binary definitions
│   └── tauri.conf.json         # Tauri v2 app manifest, window dimensions, bundle configuration
├── static/                     # Static assets (favicons, app logos)
├── .gitattributes              # Line-ending enforcement (LF for SQL migrations)
├── .gitignore                  # Git ignore patterns
├── CLI.md                      # Headless CLI usage, command reference, and daemon guide
├── eslint.config.js            # Flat ESLint configuration
├── package.json                # Frontend package scripts and dependencies
├── pnpm-lock.yaml              # Locked frontend dependency graph
├── pnpm-workspace.yaml         # Isolated pnpm workspace definition
├── README.md                   # Project overview, workflows, limits, and architecture
├── svelte.config.js            # SvelteKit configuration with adapter-static
├── TESTING.md                  # Regression test map, coverage matrix, and CI guidelines
├── tsconfig.json               # TypeScript strict compiler configuration
├── vite.config.js              # Vite bundler and development server configuration
└── vitest.config.ts            # Frontend test suite runner configuration
```

### Architectural Layout

- **Frontend (`src/`)**: Built on SvelteKit with Svelte 5 runes (`$state`, `$derived`, `$props`). Reactive application singletons in `src/lib/state/` manage local caches, optimistic mutations, and window event listeners. IPC requests flow through typed service wrappers in `src/lib/services/` using auto-generated `tauri-specta` bindings (`src/lib/generated/bindings.ts`), with mock implementations providing graceful fallbacks during browser-only development.
- **Backend Core (`src-tauri/`)**: Written in Rust and hosted within Tauri v2. Transformation logic is modularized in `src-tauri/src/pipeline/` (`Trim`, literal `Replace`, and Rust `Regex`), with bounded execution (up to 100 rules, 16 KiB per field, and 1 MiB payload limits). The background clipboard monitor polls every 250 ms with write suppression, lease-based process ownership, and best-effort pre-write re-reading.
- **Persistence (`src-tauri/src/db/`)**: Local SQLite running in WAL mode with embedded SQLx migrations. Manages persistent rule ordering, app settings, pause generation coordination, and up to 500 history records with FIFO pruning.
- **IPC, CLI & Code Generation (`src-tauri/bin/`)**: IPC commands, events, and constants are registered in a single shared `ipc_builder()` in `src-tauri/src/lib.rs`. Running `pnpm codegen` invokes `generate_bindings.rs` to produce type-safe TypeScript interfaces and bindings committed under `src/lib/generated/bindings.ts`. The headless `clipchain-cli` binary (`bin/cli.rs`) provides pipeline transformations, one-shot clipboard processing, and headless daemon operation sharing the same Rust core, verified by end-to-end integration tests in `src-tauri/tests/cli.rs`.

## Workflow

The headless `clipchain-cli` executable shares the desktop pipeline and database.
It supports piped/argument transforms, draft previews, rule management, one-shot
clipboard processing, a daemon, history, settings, and worker status. See
[CLI.md](CLI.md) for build commands, examples, and clipboard ownership behavior.

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

Rust resolves the database through a shared `dirs::data_dir()` helper matching
Tauri's `app_data_dir()`. The application
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
and cleans up async listener registration on teardown. Generated bindings in
`src/lib/generated/bindings.ts` are owned by tauri-specta; do not edit them by
hand. Their unused `Channel` import currently produces a non-fatal build warning
from the pinned generator.

```powershell
pnpm tauri build --debug --no-bundle
```

This builds a native debug executable in `src-tauri/target/debug/clipchain.exe`
without creating an installer. Frontend bundles, native executables, and release
packaging are separate verification steps.

See [TESTING.md](TESTING.md) for fresh test results, the coverage map, and
remaining native acceptance checks. Regular test suites use temporary databases
and simulated clipboard devices. Live native acceptance uses a disposable app
identity and synthetic text, with the original clipboard restored afterward.

Run `pnpm test:all` for both frontend and Rust regression suites (`pnpm test`
and `pnpm test:rust` run them separately). `pnpm ci:quick` runs the lightweight
push checks: frontend tests, types, lint, and Rust formatting. Pull requests and
manual dispatch run the full Windows, macOS, and Linux matrix, including both
test suites, native builds, and real clipboard smoke tests on disposable hosted
runners. See [TESTING.md](TESTING.md) for the coverage map, CI triggers, native
acceptance boundaries, and expectations for new features.
