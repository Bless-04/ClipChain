use super::{ClipboardAccess, Monitor};
use crate::{
    db,
    error::AppError,
    models::{AppSettings, RegexRule, ReplaceRule, Rule, TrimRule},
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
    assert_eq!(
        clipboard.writes,
        Vec::<String>::new(),
        "Starting the monitor must leave text that was already on the clipboard untouched."
    );
    clipboard.text = Some("fresh copy".into());
    let result = monitor.tick(&mut clipboard, &state).await?;
    assert!(
        result.change.is_some(),
        "A fresh external copy must produce a transformation event."
    );
    assert_eq!(
        clipboard.text.as_deref(),
        Some("fresh copy!"),
        "The first new copy must receive the saved replacement rule."
    );
    for _ in 0..3 {
        monitor.tick(&mut clipboard, &state).await?;
    }
    assert_eq!(
        clipboard.writes.len(),
        1,
        "Later polls must not transform the monitor's own output again."
    );
    assert_eq!(
        db::history::get_all(&state.pool).await?.len(),
        1,
        "Repeated polls of the same transformed text must create only one history entry."
    );
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
            .is_running,
        "A paused monitor must report that it is not running."
    );
    assert_eq!(
        clipboard.reads, reads,
        "Pausing must stop clipboard reads, not just suppress writes."
    );
    settings.is_active = true;
    db::settings::save(&state.pool, &settings).await?;
    monitor.tick(&mut clipboard, &state).await?;
    assert_eq!(
        clipboard.writes,
        Vec::<String>::new(),
        "Resuming must treat the current clipboard as the starting point and leave it unchanged."
    );
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
    assert_eq!(
        clipboard.writes,
        Vec::<String>::new(),
        "If another copy arrives during processing, discard the pending write."
    );
    assert_eq!(
        clipboard.text.as_deref(),
        Some("newer copy"),
        "The newest external copy must survive the check immediately before writing."
    );
    assert!(
        db::history::get_all(&state.pool).await?.is_empty(),
        "Discarded transformations must not appear in history."
    );
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
    assert!(
        monitor.tick(&mut clipboard, &state).await.is_err(),
        "A busy clipboard must return an error so the pending copy can be retried."
    );
    assert!(
        db::history::get_all(&state.pool).await?.is_empty(),
        "A failed clipboard write must not create a history entry for text never written."
    );
    clipboard.fail_write = false;
    monitor.tick(&mut clipboard, &state).await?;
    assert_eq!(
        clipboard.writes,
        vec!["copy!"],
        "After the clipboard becomes writable, the pending copy must be transformed once."
    );
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
    assert!(
        change.is_some_and(|change| change.entry.is_none()),
        "With recording off, report the transformation without including a history entry."
    );
    assert_eq!(
        clipboard.writes,
        vec!["copy!"],
        "Turning off history must still allow clipboard transformations."
    );
    assert!(
        db::history::get_all(&state.pool).await?.is_empty(),
        "Turning off recording must keep clipboard text out of stored history."
    );
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
    assert_eq!(
        clipboard.writes,
        Vec::<String>::new(),
        "Non-text contents and text with no matching rule must not trigger writes."
    );
    assert!(
        db::history::get_all(&state.pool).await?.is_empty(),
        "Skipped clipboard contents must not create history entries."
    );
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
    assert!(
        monitor.tick(&mut clipboard, &state).await.is_err(),
        "A failed read immediately before writing must report an error."
    );
    assert_eq!(
        clipboard.writes,
        Vec::<String>::new(),
        "If the latest clipboard cannot be checked, do not risk overwriting it."
    );
    assert!(
        db::history::get_all(&state.pool).await?.is_empty(),
        "A write skipped after a read failure must not create history."
    );
    let recovered = monitor.tick(&mut clipboard, &state).await?;
    assert!(
        recovered.status.is_running,
        "The monitor must resume running after a transient read failure clears."
    );
    assert!(
        recovered.status.last_error.is_none(),
        "A successful retry must clear the previous clipboard error."
    );
    assert_eq!(
        clipboard.writes,
        vec!["copy!"],
        "The copy pending before the read failure must be retried exactly once."
    );
    assert_eq!(
        db::history::get_all(&state.pool).await?.len(),
        1,
        "Only the successful retry must be recorded in history."
    );
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
    assert_eq!(
        clipboard.writes,
        Vec::<String>::new(),
        "Even a pause and resume between polls must leave text copied while paused untouched."
    );
    clipboard.text = Some("new copy".into());
    monitor.tick(&mut clipboard, &state).await?;
    assert_eq!(
        clipboard.writes,
        vec!["new copy!"],
        "A new copy after resuming must still be transformed."
    );
    state.pool.close().await;
    Ok(())
}

#[tokio::test]
// Keep the successive edits in one monitor session; explanatory assertion
// messages push this otherwise unchanged scenario just over the line limit.
#[allow(clippy::too_many_lines)]
async fn stored_chain_edits_toggles_and_reordering_apply_on_the_next_copy() -> TestResult {
    let directory = tempfile::tempdir()?;
    let pool = db::open(&directory.path().join("test.db")).await?;
    let trim = db::rules::save(
        &pool,
        Rule::Trim(TrimRule {
            id: 0,
            order: 0,
            name: "Trim".into(),
            is_active: true,
            trim_start: true,
            trim_end: true,
        }),
    )
    .await?;
    let replacement = db::rules::save(
        &pool,
        Rule::Replace(ReplaceRule {
            id: 0,
            order: 0,
            name: "Replace".into(),
            is_active: true,
            target: "copy".into(),
            replacement: "item42".into(),
            match_case: true,
        }),
    )
    .await?;
    let mut regex = db::rules::save(
        &pool,
        Rule::Regex(RegexRule {
            id: 0,
            order: 0,
            name: "Regex".into(),
            is_active: true,
            pattern: "^item(\\d+)$".into(),
            replacement: "number $1".into(),
        }),
    )
    .await?;
    let state = AppState::new(pool);
    let mut monitor = Monitor::default();
    let mut clipboard = FakeClipboard::default();
    monitor.tick(&mut clipboard, &state).await?;
    clipboard.text = Some("  copy  ".into());
    let tick = monitor.tick(&mut clipboard, &state).await?;
    let entry = tick
        .change
        .and_then(|change| change.entry)
        .ok_or("Expected history")?;
    assert_eq!(
        clipboard.writes,
        vec!["number 42"],
        "Saved Trim, Replace, and Regex rules must run in order on the next copy."
    );
    assert_eq!(
        entry.original_text, "  copy  ",
        "History must preserve the exact original text, including its surrounding spaces."
    );
    assert_eq!(
        entry.transformed_text, "number 42",
        "History must store the final output of the complete rule chain."
    );

    db::rules::reorder(&state.pool, &[regex.id(), replacement.id(), trim.id()]).await?;
    clipboard.text = Some(" copy ".into());
    monitor.tick(&mut clipboard, &state).await?;
    assert_eq!(
        clipboard.text.as_deref(),
        Some("item42"),
        "Moving Regex before Replace must prevent it from matching text produced later."
    );

    if let Rule::Regex(rule) = &mut regex {
        rule.is_active = false;
        rule.replacement = "edited $1".into();
    }
    db::rules::save(&state.pool, regex.clone()).await?;
    db::rules::reorder(&state.pool, &[trim.id(), replacement.id(), regex.id()]).await?;
    clipboard.text = Some("copy".into());
    monitor.tick(&mut clipboard, &state).await?;
    assert_eq!(
        clipboard.text.as_deref(),
        Some("item42"),
        "An edited but disabled Regex rule must not change the clipboard."
    );
    if let Rule::Regex(rule) = &mut regex {
        rule.is_active = true;
    }
    db::rules::save(&state.pool, regex).await?;
    clipboard.text = Some(" copy  ".into());
    monitor.tick(&mut clipboard, &state).await?;
    assert_eq!(
        clipboard.text.as_deref(),
        Some("edited 42"),
        "Re-enabling the edited Regex rule must apply its new replacement on the next copy."
    );
    assert_eq!(
        db::history::get_all(&state.pool).await?.len(),
        4,
        "Each of the four successful clipboard writes must have one history entry."
    );
    state.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn oversized_input_is_skipped_once_and_the_next_copy_recovers() -> TestResult {
    let directory = tempfile::tempdir()?;
    let state = state(&directory).await?;
    let mut monitor = Monitor::default();
    let mut clipboard = FakeClipboard::default();
    monitor.tick(&mut clipboard, &state).await?;
    clipboard.text = Some("é".repeat(crate::constants::clipboard::MAX_TEXT_BYTES / 2 + 1));
    assert!(
        matches!(
            monitor.tick(&mut clipboard, &state).await,
            Err(AppError::Validation(_))
        ),
        "Clipboard text above the byte limit must be rejected as invalid input."
    );
    assert!(
        monitor.tick(&mut clipboard, &state).await?.change.is_none(),
        "The same oversized copy must be skipped on later polls instead of repeatedly processed."
    );
    assert_eq!(
        clipboard.writes,
        Vec::<String>::new(),
        "Oversized clipboard input must never be written back."
    );
    assert!(
        db::history::get_all(&state.pool).await?.is_empty(),
        "Rejected oversized input must not be retained in history."
    );
    clipboard.text = Some("copy".into());
    assert!(
        monitor
            .tick(&mut clipboard, &state)
            .await?
            .status
            .last_error
            .is_none(),
        "A later valid copy must clear the error caused by oversized input."
    );
    assert_eq!(
        clipboard.writes,
        vec!["copy!"],
        "The monitor must recover and transform the next valid copy."
    );
    state.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn initial_read_failure_retries_baseline_before_processing_new_text() -> TestResult {
    let directory = tempfile::tempdir()?;
    let state = state(&directory).await?;
    let mut monitor = Monitor::default();
    let mut clipboard = FakeClipboard {
        text: Some("existing copy".into()),
        fail_on_read: Some(1),
        ..Default::default()
    };
    assert!(
        monitor.tick(&mut clipboard, &state).await.is_err(),
        "A failed initial clipboard read must be reported."
    );
    monitor.tick(&mut clipboard, &state).await?;
    assert_eq!(
        clipboard.writes,
        Vec::<String>::new(),
        "Retrying the initial read must establish the starting point without transforming existing text."
    );
    clipboard.text = Some("fresh copy".into());
    monitor.tick(&mut clipboard, &state).await?;
    assert_eq!(
        clipboard.writes,
        vec!["fresh copy!"],
        "After initialization recovers, the next new copy must be transformed."
    );
    state.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn peer_pause_and_resume_between_ticks_reset_the_baseline() -> TestResult {
    let directory = tempfile::tempdir()?;
    let state = state(&directory).await?;
    let peer = db::open(&directory.path().join("test.db")).await?;
    let mut monitor = Monitor::default();
    let mut clipboard = FakeClipboard::default();
    monitor.tick(&mut clipboard, &state).await?;
    db::settings::patch_monitor(&peer, Some(false), None).await?;
    clipboard.text = Some("paused copy".into());
    db::settings::patch_monitor(&peer, Some(true), None).await?;
    monitor.tick(&mut clipboard, &state).await?;
    assert_eq!(
        clipboard.writes,
        Vec::<String>::new(),
        "A pause and resume saved through another database connection must reset the monitor's starting point."
    );
    clipboard.text = Some("new copy".into());
    monitor.tick(&mut clipboard, &state).await?;
    assert_eq!(
        clipboard.writes,
        vec!["new copy!"],
        "After another client resumes monitoring, newly copied text must still be transformed."
    );
    peer.close().await;
    state.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn one_shot_transforms_existing_text_even_while_paused() -> TestResult {
    let directory = tempfile::tempdir()?;
    let state = state(&directory).await?;
    db::settings::patch_monitor(&state.pool, Some(false), None).await?;
    let mut clipboard = FakeClipboard {
        text: Some("existing copy".into()),
        ..Default::default()
    };
    let result = super::process_once(&mut clipboard, &state.pool).await?;
    assert!(
        result.is_some_and(|change| change.entry.is_some()),
        "Explicit one-shot processing must record a successful transformation even when automatic monitoring is paused."
    );
    assert_eq!(
        clipboard.writes,
        vec!["existing copy!"],
        "One-shot processing must transform text already on the clipboard."
    );
    assert_eq!(
        db::history::get_all(&state.pool).await?.len(),
        1,
        "A successful one-shot write must create exactly one history entry."
    );
    state.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn one_shot_skips_nontext_and_noops_and_obeys_history_setting() -> TestResult {
    let directory = tempfile::tempdir()?;
    let state = state(&directory).await?;
    let mut clipboard = FakeClipboard::default();
    assert!(
        super::process_once(&mut clipboard, &state.pool)
            .await?
            .is_none(),
        "One-shot processing must skip non-text clipboard contents."
    );
    clipboard.text = Some("unchanged".into());
    assert!(
        super::process_once(&mut clipboard, &state.pool)
            .await?
            .is_none(),
        "One-shot processing must skip text that the rules leave unchanged."
    );
    db::settings::patch_monitor(&state.pool, None, Some(false)).await?;
    clipboard.text = Some("copy".into());
    assert!(
        super::process_once(&mut clipboard, &state.pool)
            .await?
            .is_some_and(|change| change.entry.is_none()),
        "One-shot processing with recording disabled must report a change without a history entry."
    );
    assert_eq!(
        clipboard.writes,
        vec!["copy!"],
        "Disabling recording must not prevent an explicit clipboard transformation."
    );
    assert!(
        db::history::get_all(&state.pool).await?.is_empty(),
        "One-shot processing must honor the setting that disables retained history."
    );
    state.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn one_shot_failures_never_record_unwritten_text() -> TestResult {
    let directory = tempfile::tempdir()?;
    let state = state(&directory).await?;
    for mut clipboard in [
        FakeClipboard {
            text: Some("copy".into()),
            fail_write: true,
            ..Default::default()
        },
        FakeClipboard {
            text: Some("copy".into()),
            change_on_read: Some(2),
            ..Default::default()
        },
        FakeClipboard {
            text: Some("copy".into()),
            fail_on_read: Some(2),
            ..Default::default()
        },
        FakeClipboard {
            text: Some("x".repeat(crate::constants::clipboard::MAX_TEXT_BYTES + 1)),
            ..Default::default()
        },
    ] {
        assert!(
            super::process_once(&mut clipboard, &state.pool)
                .await
                .is_err(),
            "Unsafe or unsuccessful one-shot operations must return an error."
        );
        assert_eq!(
            clipboard.writes,
            Vec::<String>::new(),
            "A failed write, changed clipboard, failed reread, or oversized input must not write transformed text."
        );
    }
    assert!(
        db::history::get_all(&state.pool).await?.is_empty(),
        "Failed one-shot operations must not create history for unwritten text."
    );
    state.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn shutdown_while_waiting_for_the_gate_prevents_clipboard_access() -> TestResult {
    let directory = tempfile::tempdir()?;
    let state = std::sync::Arc::new(state(&directory).await?);
    let gate = state.gate.lock().await;
    let worker_state = state.clone();
    let tick = tokio::spawn(async move {
        let mut clipboard = FakeClipboard {
            text: Some("copy".into()),
            ..Default::default()
        };
        let tick = Monitor::default()
            .tick(&mut clipboard, &worker_state)
            .await?;
        Ok::<_, AppError>((tick.status, clipboard.reads, clipboard.writes))
    });
    tokio::task::yield_now().await;
    state
        .shutdown
        .store(true, std::sync::atomic::Ordering::Relaxed);
    drop(gate);
    let (status, reads, writes) = tick.await??;
    assert!(
        !status.is_running,
        "A poll released after shutdown was requested must report a stopped worker."
    );
    assert_eq!(
        reads, 0,
        "A poll waiting during shutdown must exit before reading the clipboard."
    );
    assert_eq!(
        writes,
        Vec::<String>::new(),
        "A poll waiting during shutdown must not write to the clipboard."
    );
    state.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn joinable_worker_delivers_errors_and_releases_ownership_after_shutdown() -> TestResult {
    use std::sync::{Arc, Mutex};
    #[derive(Clone)]
    struct Listener(Arc<Mutex<Vec<crate::models::events::MonitorStatus>>>);
    impl super::ClipboardListener for Listener {
        fn status_changed(
            &self,
            status: &crate::models::events::MonitorStatus,
        ) -> Result<(), AppError> {
            self.0
                .lock()
                .map_err(|error| AppError::Internal(error.to_string()))?
                .push(status.clone());
            Ok(())
        }
        fn transformed(&self, _: &crate::models::events::ClipboardChanged) -> Result<(), AppError> {
            Ok(())
        }
    }
    let directory = tempfile::tempdir()?;
    let state = Arc::new(state(&directory).await?);
    let lock = directory.path().join("clipboard.lock");
    let listener = Listener(Arc::new(Mutex::new(Vec::new())));
    let lease =
        super::ownership::ClipboardLease::acquire(&lock, &directory.path().join("test.db"))?;
    let worker = super::start_with_device(listener.clone(), state.clone(), lease, || {
        Err::<FakeClipboard, _>(AppError::Clipboard("simulated device lock".into()))
    })?;
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if listener
                .0
                .lock()
                .map_err(|error| AppError::Internal(error.to_string()))?
                .iter()
                .any(|status| status.last_error.is_some())
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        Ok::<_, AppError>(())
    })
    .await??;
    assert!(
        super::ownership::inspect(&lock)?.has_owner,
        "The worker must retain clipboard ownership while reporting a device error."
    );
    worker.stop()?;
    assert!(
        worker.is_finished(),
        "Stopping the worker must wait until its background thread has finished."
    );
    assert!(
        !super::ownership::inspect(&lock)?.has_owner,
        "After the worker stops, another process must be able to acquire clipboard ownership."
    );
    state.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn shutdown_during_prewrite_read_discards_the_pending_write() -> TestResult {
    struct ShutdownClipboard<'a> {
        shutdown: &'a std::sync::atomic::AtomicBool,
        reads: usize,
        writes: usize,
    }
    impl ClipboardAccess for ShutdownClipboard<'_> {
        fn read(&mut self) -> Result<Option<String>, AppError> {
            self.reads += 1;
            if self.reads == 3 {
                self.shutdown
                    .store(true, std::sync::atomic::Ordering::Relaxed);
            }
            Ok(Some(
                if self.reads == 1 { "baseline" } else { "copy" }.into(),
            ))
        }
        fn write(&mut self, _: &str) -> Result<(), AppError> {
            self.writes += 1;
            Ok(())
        }
    }
    let directory = tempfile::tempdir()?;
    let state = state(&directory).await?;
    let mut clipboard = ShutdownClipboard {
        shutdown: &state.shutdown,
        reads: 0,
        writes: 0,
    };
    let mut monitor = Monitor::default();
    monitor.tick(&mut clipboard, &state).await?;
    let tick = monitor.tick(&mut clipboard, &state).await?;
    assert!(
        !tick.status.is_running,
        "Shutdown detected immediately before a write must mark the monitor as stopped."
    );
    assert!(
        tick.change.is_none(),
        "An interrupted poll must not report a transformation that never happened."
    );
    assert_eq!(
        clipboard.writes, 0,
        "A shutdown request during the final clipboard read must prevent the pending write."
    );
    assert!(
        db::history::get_all(&state.pool).await?.is_empty(),
        "An interrupted write must not create a history entry."
    );
    state.pool.close().await;
    Ok(())
}
