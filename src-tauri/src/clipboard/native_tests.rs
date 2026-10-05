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
    assert!(require_hosted_runner(Some("true"), Some("github-hosted"), Some("1")).is_ok());
    for (actions, environment, opt_in) in [
        (None, Some("github-hosted"), Some("1")),
        (Some("false"), Some("github-hosted"), Some("1")),
        (Some("true"), None, Some("1")),
        (Some("true"), Some("self-hosted"), Some("1")),
        (Some("true"), Some("github-hosted"), None),
        (Some("true"), Some("github-hosted"), Some("0")),
    ] {
        assert!(require_hosted_runner(actions, environment, opt_in).is_err());
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
    assert!(tick_with_retry(&mut monitor, &mut device, &state)
        .await?
        .change
        .is_none());
    assert_eq!(
        read_external(&mut external).await?.as_deref(),
        Some("baseline copy")
    );

    let original = " \tcafé 😀\ncopy 42 \n";
    let expected = "café 😀\ncopy! [42]";
    write_external(&mut external, original).await?;
    let tick = tick_with_retry(&mut monitor, &mut device, &state).await?;
    assert!(tick.status.is_running);
    assert!(tick.status.last_error.is_none());
    let entry = tick
        .change
        .and_then(|change| change.entry)
        .ok_or("Expected recorded native transformation")?;
    assert_eq!(entry.original_text, original);
    assert_eq!(entry.transformed_text, expected);
    assert_eq!(
        read_external(&mut external).await?.as_deref(),
        Some(expected)
    );
    for _ in 0..3 {
        assert!(tick_with_retry(&mut monitor, &mut device, &state)
            .await?
            .change
            .is_none());
    }
    assert_eq!(db::history::get_all(&state.pool).await?.len(), 1);

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
            .is_running
    );
    assert_eq!(
        read_external(&mut external).await?.as_deref(),
        Some("paused copy")
    );
    state.persist_settings(&AppSettings::default()).await?;
    assert!(tick_with_retry(&mut monitor, &mut device, &state)
        .await?
        .change
        .is_none());
    assert_eq!(
        read_external(&mut external).await?.as_deref(),
        Some("paused copy")
    );

    state
        .persist_settings(&AppSettings {
            record_history: false,
            ..Default::default()
        })
        .await?;
    write_external(&mut external, "  next copy 7  ").await?;
    let tick = tick_with_retry(&mut monitor, &mut device, &state).await?;
    assert!(tick.change.is_some_and(|change| change.entry.is_none()));
    assert_eq!(
        read_external(&mut external).await?.as_deref(),
        Some("next copy! [7]")
    );
    assert_eq!(db::history::get_all(&state.pool).await?.len(), 1);
    db::history::clear(&state.pool).await?;
    assert!(db::history::get_all(&state.pool).await?.is_empty());
    assert_eq!(db::rules::get_all(&state.pool).await?.len(), 4);
    assert!(!db::settings::get(&state.pool).await?.record_history);
    state.pool.close().await;
    Ok(())
}
