# Regression testing

Run from the repository root with the prerequisites in [README.md](README.md):

```powershell
pnpm test:all
```

`pnpm test` runs Vitest and Testing Library; `pnpm test:rust` runs Rust tests
using the committed Cargo lockfile. `pnpm test:watch` watches frontend changes.
Rust tests use temporary SQLite databases and simulated clipboard devices.
Startup compensation tests inject the OS operation and use real database saves
and failures. These suites do not read or write the user's clipboard, database,
or startup registration.

## Feature coverage map

| Feature or contract | Automated regressions |
| --- | --- |
| Trim, literal Replace, Regex, Unicode, captures, output limits | `src-tauri/src/pipeline/transformer.rs` |
| Ordered chaining, disabled rules, abort on error | `src-tauri/src/pipeline/service.rs`; `src-tauri/src/clipboard/tests.rs` exercises saved Trim → Replace → Regex, edits, toggles, and reordering |
| Rule names, safe IDs, positions, empty fields, UTF-8 limits, invalid Rust regex | `src-tauri/src/pipeline/validation.rs`; `src/lib/rules/draft.test.ts` |
| Native draft preview, inactive drafts, input limits | `src-tauri/src/commands/rules.rs`; `src/lib/components/components.test.ts` |
| Rule CRUD, canonical IDs/order, invalid reorder rollback, rule-count limit | `src-tauri/src/db/tests.rs`; `src/lib/state/state.test.ts`; `src/routes/rules/rules-page.test.ts` |
| Restart persistence of edited rule variants, settings, history; migration checksums | `src-tauri/src/db/tests.rs` |
| Clipboard baseline, own-write suppression, pause/resume, newer-copy guard, transient read/write failures, oversized-copy recovery | `src-tauri/src/clipboard/tests.rs`; `src-tauri/src/db/tests.rs` |
| Disabled recording, bounded newest-first history, isolated clear, failed history inserts and subsequent recovery | `src-tauri/src/clipboard/tests.rs`; `src-tauri/src/db/tests.rs` |
| Startup enable/disable coordination, unchanged registration, OS failure, database failure, compensation in both directions, failed compensation | `src-tauri/src/commands/settings/tests.rs` |
| Settings defaults and legacy serialization; every typed IPC error kind | `src-tauri/src/models/settings.rs`; `src-tauri/src/error.rs` |
| Typed services, native availability, listener cleanup and partial registration failure | `src/lib/services/services.test.ts` |
| Confirmed state, stale reads, event coalescing, mutation failure/retry, canonical responses, live events, reconnect and teardown | `src/lib/state/state.test.ts` |
| All three editors, visual regex builder, preview races, validation, failed-save drafts, busy controls | `src/lib/components/components.test.ts`; `src/routes/rules/rules-page.test.ts` |
| Dashboard counts, recent entries, pause/resume, worker health, failures and retries | `src/routes/dashboard.test.ts` |
| History safe text, whitespace/change highlighting, Unicode excerpts, pagination and shrinking pages, clear confirmation/cancel/retry | `src/lib/components/components.test.ts`; `src/lib/components/panels.test.ts`; `src/lib/rules/draft.test.ts` |
| Settings toggles, complete saves, canonical readback, initial-load recovery, browser controls | `src/lib/components/components.test.ts`; `src/lib/components/panels.test.ts` |
| Navigation destinations/current route, browser notice, visible reconnect, layout disposal, modal cancel/busy behavior and opener focus | `src/routes/layout.test.ts`; `src/routes/rules/rules-page.test.ts`; `src/lib/components/panels.test.ts` |
| Real OS clipboard with saved rules, Unicode/multiline text, ordered chaining, disabled rules, own-write suppression, pause/resume, disabled history, isolated clearing | `src-tauri/src/clipboard/native_tests.rs` (explicit full CI only) |

## CI workflows

| Workflow | Triggers | Checks |
| --- | --- | --- |
| Quick checks (`.github/workflows/checks.yml`) | Every push to any branch or tag; manual dispatch | One Ubuntu job: locked pnpm install, frontend type checking, ESLint, all frontend regressions, Rust formatting, whitespace |
| Cross-platform checks (`.github/workflows/cross-platform.yml`) | Pull requests, including new commits; manual dispatch | Independent Windows, macOS, and Ubuntu 24.04 jobs: generated bindings, frontend types/lint/tests, all Rust regressions, formatting, Clippy, real OS clipboard smoke, and native debug application build |

Quick checks use the committed bindings and avoid Rust compilation and Tauri
system dependencies. Run the same checks locally with `pnpm ci:quick`. Backend
behavior tests run in the full workflow, so use `pnpm test:rust` locally when
changing Rust behavior between pull requests.

The full matrix caches Rust dependencies and the pnpm store. A failing platform
does not cancel the others. It installs Tauri's Linux system dependencies and
runs the Linux clipboard smoke in an X11 virtual desktop using Xvfb. See the
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/#linux) for the
system packages. The macOS job uses the architecture of `macos-latest`; this
matrix does not separately verify Intel macOS or Linux Wayland.

Each platform runs the same behavioral assertions against its own real OS
clipboard adapter and a temporary database. The smoke uses separate clipboard
handles for the external copy and monitor, directly drives monitor ticks, and
checks saved history. It does not launch the native window or exercise the
polling thread lifecycle. `pnpm test:native` selects this test explicitly; normal
Rust tests ignore it. It requires `GITHUB_ACTIONS=true`,
`RUNNER_ENVIRONMENT=github-hosted`, and `CLIPCHAIN_NATIVE_TESTS=1` before accessing
the clipboard. These runner values are provided by [GitHub Actions](https://docs.github.com/en/actions/reference/workflows-and-actions/variables#default-environment-variables).
Only the native smoke steps opt in, so ordinary local tests and self-hosted
runners do not change their clipboards.

The native builds use `--debug --no-bundle --ci -- --locked`. They compile the
actual application with embedded frontend assets without generating installers
or signed packages. The workflows become active once committed and pushed;
their first hosted runs establish actual matrix results. Neither workflow has
been run remotely in this session. Required merge checks remain a repository
setting.

## Future changes

Every feature addition or behavior change should include a regression at the
layer that owns the behavior. Cover its normal use and a meaningful failure or
boundary case. For bug fixes, reproduce the observed failure in a test before
fixing it. Test observable outputs, confirmed state, and user interactions;
avoid snapshots of incidental markup or assertions on private implementation.

For a new rule, add transformation, validation, persistence, preview, pipeline,
and editor interaction cases. For an IPC or setting change, regenerate bindings
and check command forwarding, canonical responses, failure recovery, events,
and compatibility defaults where applicable. Keep the coverage map current.

Run the relevant focused tests during development, then `pnpm test:all`,
`pnpm check`, `pnpm lint`, `pnpm build`, Rust formatting, and Clippy before
completing a change. The full workflow performs these checks on all three
platforms and generates bindings from the shared registry before checking the
frontend. Bindings are not compared byte-for-byte because the pinned generator
can reorder constants.

## Native acceptance boundaries

Regular feature regressions use simulated clipboard devices; the full CI smoke
adds real clipboard adapter checks on hosted desktops. This does not establish
complete desktop UI integration or webview layout. The [handoff](HANDOFF_PROMPT.md) tracks native observations and
remaining acceptance work. These checks still require a disposable application
identity and synthetic fixtures:

- Clipboard locks, timed polling, and shutdown during a tick.
- Startup registration readback and launch at OS sign-in.
- Second-instance window restoration and focus.
- Native webview focus trapping, keyboard behavior, and responsive layout.
- Installation, signing, and release packaging when requested.

Preserve the original clipboard and registration, use temporary app data, and
restore the prior state after native acceptance. A fake clipboard, startup
callback, or jsdom dialog shim is evidence for the application behavior tested,
not for the corresponding OS operation. Vitest's `$app` stand-ins only supply
route paths and current-page context; navigation rendering is tested without
simulating SvelteKit routing.
