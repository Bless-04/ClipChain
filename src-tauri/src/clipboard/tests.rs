use super::{ClipboardAccess, Monitor};
use crate::{
    db,
    error::AppError,
    models::{AppSettings, ReplaceRule, Rule},
    state::AppState,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Default)]
struct FakeClipboard {
    text: Option<String>,
    writes: Vec<String>,
    reads: usize,
    fail_write: bool,
    change_on_read: Option<usize>,
    fail_on_read: Option<usize>,
}
impl ClipboardAccess for FakeClipboard {
    fn read(&mut self) -> Result<Option<String>, AppError> {
        self.reads += 1;
        if self.fail_on_read == Some(self.reads) {
            return Err(AppError::Clipboard("temporarily locked".into()));
        }
        if self.change_on_read == Some(self.reads) {
            self.text = Some("newer copy".into());
        }
        Ok(self.text.clone())
    }
    fn write(&mut self, text: &str) -> Result<(), AppError> {
        if self.fail_write {
            return Err(AppError::Clipboard("busy".into()));
        }
        self.text = Some(text.into());
        self.writes.push(text.into());
        Ok(())
    }
}
async fn state(directory: &tempfile::TempDir) -> Result<AppState, AppError> {
    let pool = db::open(&directory.path().join("test.db")).await?;
    // Deliberately non-idempotent: processing our output twice would append again.
    db::rules::save(
        &pool,
        Rule::Replace(ReplaceRule {
            id: 0,
            name: "Append".into(),
            order: 0,
            is_active: true,
            target: "copy".into(),
            replacement: "copy!".into(),
            match_case: true,
        }),
    )
    .await?;
    Ok(AppState::new(pool))
}

#[tokio::test]
async fn startup_skips_existing_text_and_own_writes_are_not_reprocessed() -> TestResult {
    let directory = tempfile::tempdir()?;
    let state = state(&directory).await?;
    let mut clipboard = FakeClipboard {
        text: Some("existing copy".into()),
        ..Default::default()
    };
    let mut monitor = Monitor::default();
    monitor.tick(&mut clipboard, &state).await?;
    assert!(clipboard.writes.is_empty());
    clipboard.text = Some("fresh copy".into());
    let result = monitor.tick(&mut clipboard, &state).await?;
    assert!(result.change.is_some());
    assert_eq!(clipboard.text.as_deref(), Some("fresh copy!"));
    for _ in 0..3 {
        monitor.tick(&mut clipboard, &state).await?;
    }
    assert_eq!(clipboard.writes.len(), 1);
    assert_eq!(db::history::get_all(&state.pool).await?.len(), 1);
    state.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn pause_stops_reads_and_resume_does_not_transform_the_current_clipboard() -> TestResult {
    let directory = tempfile::tempdir()?;
    let state = state(&directory).await?;
    let mut clipboard = FakeClipboard::default();
    let mut monitor = Monitor::default();
    monitor.tick(&mut clipboard, &state).await?;
    let mut settings = AppSettings {
        is_active: false,
        ..Default::default()
    };
    db::settings::save(&state.pool, &settings).await?;
    let reads = clipboard.reads;
    clipboard.text = Some("paused copy".into());
    assert!(
        !monitor
            .tick(&mut clipboard, &state)
            .await?
            .status
            .is_running
    );
    assert_eq!(clipboard.reads, reads);
    settings.is_active = true;
    db::settings::save(&state.pool, &settings).await?;
    monitor.tick(&mut clipboard, &state).await?;
    assert!(clipboard.writes.is_empty());
    state.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn changed_clipboard_is_not_overwritten() -> TestResult {
    let directory = tempfile::tempdir()?;
    let state = state(&directory).await?;
    let mut clipboard = FakeClipboard::default();
    let mut monitor = Monitor::default();
    monitor.tick(&mut clipboard, &state).await?;
    clipboard.text = Some("old copy".into());
    clipboard.change_on_read = Some(3);
    monitor.tick(&mut clipboard, &state).await?;
    assert!(clipboard.writes.is_empty());
    assert_eq!(clipboard.text.as_deref(), Some("newer copy"));
    assert!(db::history::get_all(&state.pool).await?.is_empty());
    state.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn failed_write_is_retried_without_phantom_history() -> TestResult {
    let directory = tempfile::tempdir()?;
    let state = state(&directory).await?;
    let mut clipboard = FakeClipboard::default();
    let mut monitor = Monitor::default();
    monitor.tick(&mut clipboard, &state).await?;
    clipboard.text = Some("copy".into());
    clipboard.fail_write = true;
    assert!(monitor.tick(&mut clipboard, &state).await.is_err());
    assert!(db::history::get_all(&state.pool).await?.is_empty());
    clipboard.fail_write = false;
    monitor.tick(&mut clipboard, &state).await?;
    assert_eq!(clipboard.writes, vec!["copy!"]);
    state.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn disabled_history_omits_text_but_keeps_transformations_working() -> TestResult {
    let directory = tempfile::tempdir()?;
    let state = state(&directory).await?;
    db::settings::save(
        &state.pool,
        &AppSettings {
            record_history: false,
            ..Default::default()
        },
    )
    .await?;
    let mut clipboard = FakeClipboard::default();
    let mut monitor = Monitor::default();
    monitor.tick(&mut clipboard, &state).await?;
    clipboard.text = Some("copy".into());
    let change = monitor.tick(&mut clipboard, &state).await?.change;
    assert!(change.is_some_and(|change| change.entry.is_none()));
    assert_eq!(clipboard.writes, vec!["copy!"]);
    assert!(db::history::get_all(&state.pool).await?.is_empty());
    state.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn nontext_and_unchanged_text_never_create_history() -> TestResult {
    let directory = tempfile::tempdir()?;
    let state = state(&directory).await?;
    let mut clipboard = FakeClipboard::default();
    let mut monitor = Monitor::default();
    monitor.tick(&mut clipboard, &state).await?;
    clipboard.text = Some("nothing matches".into());
    monitor.tick(&mut clipboard, &state).await?;
    clipboard.text = None;
    monitor.tick(&mut clipboard, &state).await?;
    assert!(clipboard.writes.is_empty());
    assert!(db::history::get_all(&state.pool).await?.is_empty());
    state.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn failed_prewrite_read_retries_the_pending_copy() -> TestResult {
    let directory = tempfile::tempdir()?;
    let state = state(&directory).await?;
    let mut clipboard = FakeClipboard::default();
    let mut monitor = Monitor::default();
    monitor.tick(&mut clipboard, &state).await?;
    clipboard.text = Some("copy".into());
    clipboard.fail_on_read = Some(3);
    assert!(monitor.tick(&mut clipboard, &state).await.is_err());
    assert!(clipboard.writes.is_empty());
    assert!(db::history::get_all(&state.pool).await?.is_empty());
    let recovered = monitor.tick(&mut clipboard, &state).await?;
    assert!(recovered.status.is_running);
    assert!(recovered.status.last_error.is_none());
    assert_eq!(clipboard.writes, vec!["copy!"]);
    assert_eq!(db::history::get_all(&state.pool).await?.len(), 1);
    state.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn pause_and_resume_between_polls_still_reset_the_baseline() -> TestResult {
    let directory = tempfile::tempdir()?;
    let state = state(&directory).await?;
    let mut clipboard = FakeClipboard::default();
    let mut monitor = Monitor::default();
    monitor.tick(&mut clipboard, &state).await?;
    state
        .persist_settings(&AppSettings {
            is_active: false,
            ..Default::default()
        })
        .await?;
    clipboard.text = Some("paused copy".into());
    state.persist_settings(&AppSettings::default()).await?;
    monitor.tick(&mut clipboard, &state).await?;
    assert!(clipboard.writes.is_empty());
    clipboard.text = Some("new copy".into());
    monitor.tick(&mut clipboard, &state).await?;
    assert_eq!(clipboard.writes, vec!["new copy!"]);
    state.pool.close().await;
    Ok(())
}
