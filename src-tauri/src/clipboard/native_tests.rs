//! Explicit native acceptance for disposable GitHub-hosted desktops only.

use super::{ClipboardAccess, Monitor, Tick};
use crate::{
    db,
    error::AppError,
    models::{AppSettings, RegexRule, ReplaceRule, Rule, TrimRule},
    state::AppState,
};
use std::time::Duration;

type TestResult = Result<(), Box<dyn std::error::Error>>;
const RETRIES: usize = 20;
const RETRY_DELAY: Duration = Duration::from_millis(50);

fn require_hosted_runner(
    actions: Option<&str>,
    environment: Option<&str>,
    opt_in: Option<&str>,
) -> Result<(), &'static str> {
    if actions == Some("true") && environment == Some("github-hosted") && opt_in == Some("1") {
        Ok(())
    } else {
        Err("Native clipboard smoke requires a GitHub-hosted runner and CLIPCHAIN_NATIVE_TESTS=1")
    }
}

#[test]
fn native_clipboard_requires_explicit_disposable_runner() {
    assert!(
        require_hosted_runner(Some("true"), Some("github-hosted"), Some("1")).is_ok(),
        "The native clipboard smoke test may run only on an explicitly opted-in disposable hosted runner."
    );
    for (actions, environment, opt_in) in [
        (None, Some("github-hosted"), Some("1")),
        (Some("false"), Some("github-hosted"), Some("1")),
        (Some("true"), None, Some("1")),
        (Some("true"), Some("self-hosted"), Some("1")),
        (Some("true"), Some("github-hosted"), None),
        (Some("true"), Some("github-hosted"), Some("0")),
    ] {
        assert!(
            require_hosted_runner(actions, environment, opt_in).is_err(),
            "The native test must reject incomplete or unsafe runner settings: actions={actions:?}, environment={environment:?}, opt_in={opt_in:?}."
        );
    }
}

async fn write_external(device: &mut arboard::Clipboard, text: &str) -> Result<(), AppError> {
    for attempt in 0..RETRIES {
        match device.write(text) {
            Ok(()) => return Ok(()),
            Err(error) if attempt + 1 == RETRIES => return Err(error),
            Err(_) => tokio::time::sleep(RETRY_DELAY).await,
        }
    }
    Err(AppError::Clipboard(
        "Native clipboard write timed out".into(),
    ))
}

async fn read_external(device: &mut arboard::Clipboard) -> Result<Option<String>, AppError> {
    for attempt in 0..RETRIES {
        match device.read() {
            Err(error) if attempt + 1 == RETRIES => return Err(error),
            Err(_) => tokio::time::sleep(RETRY_DELAY).await,
            result => return result,
        }
    }
    Err(AppError::Clipboard(
        "Native clipboard read timed out".into(),
    ))
}

async fn tick_with_retry(
    monitor: &mut Monitor,
    device: &mut arboard::Clipboard,
    state: &AppState,
) -> Result<Tick, AppError> {
    for attempt in 0..RETRIES {
        match monitor.tick(device, state).await {
            Err(AppError::Clipboard(_)) if attempt + 1 < RETRIES => {
                tokio::time::sleep(RETRY_DELAY).await;
            }
            result => return result,
        }
    }
    Err(AppError::Clipboard(
        "Native clipboard tick timed out".into(),
    ))
}

async fn pipeline_state(directory: &tempfile::TempDir) -> Result<AppState, AppError> {
    let pool = db::open(&directory.path().join("native-smoke.db")).await?;
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
            name: "Append".into(),
            is_active: true,
            target: "copy".into(),
            replacement: "copy!".into(),
            match_case: true,
        }),
        Rule::Regex(RegexRule {
            id: 0,
            order: 0,
            name: "Numbers".into(),
            is_active: true,
            pattern: r"\d+".into(),
            replacement: "[$0]".into(),
        }),
        Rule::Replace(ReplaceRule {
            id: 0,
            order: 0,
            name: "Disabled".into(),
            is_active: false,
            target: "café".into(),
            replacement: "unexpected".into(),
            match_case: true,
        }),
    ] {
        db::rules::save(&pool, rule).await?;
    }
    Ok(AppState::new(pool))
}

// Kept out of ordinary local suites: this test intentionally replaces clipboard
// contents on an ephemeral CI desktop. The guard runs before any OS access.
#[tokio::test]
#[ignore = "Requires an explicitly opted-in GitHub-hosted desktop; use pnpm test:native in CI"]
// Keep the guarded native lifecycle in one scenario so its ordered clipboard
// steps and their failure explanations can be read together.
#[allow(clippy::too_many_lines)]
async fn native_clipboard_pipeline_smoke() -> TestResult {
    require_hosted_runner(
        std::env::var("GITHUB_ACTIONS").ok().as_deref(),
        std::env::var("RUNNER_ENVIRONMENT").ok().as_deref(),
        std::env::var("CLIPCHAIN_NATIVE_TESTS").ok().as_deref(),
    )?;
    let directory = tempfile::tempdir()?;
    let state = pipeline_state(&directory).await?;
    // Separate handles exercise clipboard reads from another OS clipboard owner.
    let mut external = arboard::Clipboard::new()?;
    let mut device = arboard::Clipboard::new()?;
    let mut monitor = Monitor::default();
    write_external(&mut external, "baseline copy").await?;
    assert!(
        tick_with_retry(&mut monitor, &mut device, &state)
            .await?
            .change
            .is_none(),
        "Starting a native monitor must not transform text already on the clipboard."
    );
    assert_eq!(
        read_external(&mut external).await?.as_deref(),
        Some("baseline copy"),
        "The initial native clipboard contents must remain unchanged while establishing the starting point."
    );

    let original = " \tcafé 😀\ncopy 42 \n";
    let expected = "café 😀\ncopy! [42]";
    write_external(&mut external, original).await?;
    let tick = tick_with_retry(&mut monitor, &mut device, &state).await?;
    assert!(
        tick.status.is_running,
        "A successful native clipboard poll must report a running monitor."
    );
    assert!(
        tick.status.last_error.is_none(),
        "A successful native transformation must not leave a worker error."
    );
    let entry = tick
        .change
        .and_then(|change| change.entry)
        .ok_or("Expected recorded native transformation")?;
    assert_eq!(
        entry.original_text, original,
        "Native history must retain the original Unicode and multiline text exactly."
    );
    assert_eq!(
        entry.transformed_text, expected,
        "Native history must store the final Trim, Replace, and Regex output."
    );
    assert_eq!(
        read_external(&mut external).await?.as_deref(),
        Some(expected),
        "An independent clipboard handle must read the transformed text written by the monitor."
    );
    for _ in 0..3 {
        assert!(
            tick_with_retry(&mut monitor, &mut device, &state)
                .await?
                .change
                .is_none(),
            "Later native polls must not treat the monitor's own write as a new copy."
        );
    }
    assert_eq!(
        db::history::get_all(&state.pool).await?.len(),
        1,
        "One external copy must create only one history entry even after repeated native polls."
    );

    state
        .persist_settings(&AppSettings {
            is_active: false,
            ..Default::default()
        })
        .await?;
    write_external(&mut external, "paused copy").await?;
    assert!(
        !tick_with_retry(&mut monitor, &mut device, &state)
            .await?
            .status
            .is_running,
        "Pausing must make the native monitor report that it is stopped."
    );
    assert_eq!(
        read_external(&mut external).await?.as_deref(),
        Some("paused copy"),
        "Text copied while paused must remain unchanged."
    );
    state.persist_settings(&AppSettings::default()).await?;
    assert!(
        tick_with_retry(&mut monitor, &mut device, &state)
            .await?
            .change
            .is_none(),
        "Resuming must establish a fresh starting point without transforming the paused copy."
    );
    assert_eq!(
        read_external(&mut external).await?.as_deref(),
        Some("paused copy"),
        "The copy made during the pause must survive the first poll after resuming."
    );

    state
        .persist_settings(&AppSettings {
            record_history: false,
            ..Default::default()
        })
        .await?;
    write_external(&mut external, "  next copy 7  ").await?;
    let tick = tick_with_retry(&mut monitor, &mut device, &state).await?;
    assert!(
        tick.change.is_some_and(|change| change.entry.is_none()),
        "With recording disabled, native processing must report a change without a history entry."
    );
    assert_eq!(
        read_external(&mut external).await?.as_deref(),
        Some("next copy! [7]"),
        "Native transformations must continue after history recording is disabled."
    );
    assert_eq!(
        db::history::get_all(&state.pool).await?.len(),
        1,
        "Disabling recording must preserve old history without adding new entries."
    );
    db::history::clear(&state.pool).await?;
    assert!(
        db::history::get_all(&state.pool).await?.is_empty(),
        "Clearing native test history must remove all retained entries."
    );
    assert_eq!(
        db::rules::get_all(&state.pool).await?.len(),
        4,
        "Clearing history must preserve all four saved pipeline rules."
    );
    assert!(
        !db::settings::get(&state.pool).await?.record_history,
        "Clearing history must not re-enable recording."
    );
    state.pool.close().await;
    Ok(())
}
