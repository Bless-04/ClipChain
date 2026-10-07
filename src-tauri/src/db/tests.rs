use super::{history, open, rules, settings};
use crate::{
    constants,
    error::AppError,
    models::{AppSettings, RegexRule, ReplaceRule, Rule, TrimRule},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn shared_database_path_uses_the_tauri_identity() -> TestResult {
    let config: serde_json::Value = serde_json::from_str(include_str!("../../tauri.conf.json"))?;
    assert_eq!(
        config["identifier"],
        constants::app::IDENTIFIER,
        "The shared application identifier must match Tauri so the CLI and desktop locate the same data."
    );
    let data = dirs::data_dir().ok_or("data directory unavailable")?;
    assert_eq!(
        super::default_database_path()?,
        data.join(constants::app::IDENTIFIER)
            .join(constants::db::DB_FILE_NAME),
        "The default database must live under the application's platform data directory."
    );
    Ok(())
}

#[tokio::test]
async fn concurrent_peer_settings_patches_preserve_each_others_fields() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("test.db");
    let first = open(&path).await?;
    let second = open(&path).await?;
    settings::save(
        &first,
        &AppSettings {
            launch_at_startup: true,
            ..Default::default()
        },
    )
    .await?;
    let (active, history) = tokio::join!(
        settings::patch_monitor(&first, Some(false), None),
        settings::patch_monitor(&second, None, Some(false))
    );
    active?;
    history?;
    let (configuration, generation) = settings::get_with_generation(&first).await?;
    assert!(
        !configuration.is_active,
        "Concurrent settings writes must preserve the request to pause monitoring."
    );
    assert!(
        !configuration.record_history,
        "Concurrent settings writes must preserve the request to disable history."
    );
    assert!(
        configuration.launch_at_startup,
        "Changing monitoring and recording must preserve the saved startup preference."
    );
    assert!(
        generation > 0,
        "Pausing must advance the durable marker used to detect pauses between clipboard polls."
    );
    first.close().await;
    second.close().await;
    Ok(())
}

#[tokio::test]
async fn database_created_with_original_lf_migration_survives_windows_checkout() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("existing.db");
    let migrations = directory.path().join("original-migrations");
    std::fs::create_dir(&migrations)?;
    // Model the original release, even if a Windows checkout converts the source to CRLF.
    std::fs::write(
        migrations.join("0001_initial.sql"),
        include_str!("migrations/0001_initial.sql").replace("\r\n", "\n"),
    )?;
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true);
    let original = sqlx::SqlitePool::connect_with(options).await?;
    sqlx::migrate::Migrator::new(migrations)
        .await?
        .run(&original)
        .await?;
    let saved = rules::save(&original, draft("Keep existing rule")).await?;
    history::insert(&original, " before ", "before").await?;
    original.close().await;

    let reopened = open(&path).await?;
    assert_eq!(
        rules::get_all(&reopened).await?[0].id(),
        saved.id(),
        "Upgrading a database created with the original migration must preserve existing rule IDs."
    );
    assert_eq!(
        history::get_all(&reopened).await?.len(),
        1,
        "Upgrading the schema must preserve existing history."
    );
    reopened.close().await;
    Ok(())
}

#[tokio::test]
async fn failed_rule_read_retries_pending_clipboard_text_after_repair() -> TestResult {
    use crate::{
        clipboard::{ClipboardAccess, Monitor},
        state::AppState,
    };
    struct TextClipboard(String);
    impl ClipboardAccess for TextClipboard {
        fn read(&mut self) -> Result<Option<String>, AppError> {
            Ok(Some(self.0.clone()))
        }
        fn write(&mut self, text: &str) -> Result<(), AppError> {
            self.0 = text.into();
            Ok(())
        }
    }
    let directory = tempfile::tempdir()?;
    let pool = open(&directory.path().join(constants::db::DB_FILE_NAME)).await?;
    let saved = rules::save(&pool, draft("Trim")).await?;
    let state = AppState::new(pool);
    let mut clipboard = TextClipboard("baseline".into());
    let mut monitor = Monitor::default();
    monitor.tick(&mut clipboard, &state).await?;
    clipboard.0 = "  pending copy  ".into();
    // Inject a stored-data read failure without touching the real clipboard.
    sqlx::query("UPDATE rules SET payload = '{}'")
        .execute(&state.pool)
        .await?;
    assert!(
        monitor.tick(&mut clipboard, &state).await.is_err(),
        "Unreadable saved rules must report an error instead of using an incomplete pipeline."
    );
    assert_eq!(
        clipboard.0, "  pending copy  ",
        "A rule-read failure must leave the original clipboard text untouched."
    );
    assert!(
        history::get_all(&state.pool).await?.is_empty(),
        "A rule-read failure must not record a transformation that did not happen."
    );
    rules::save(&state.pool, saved).await?;
    monitor.tick(&mut clipboard, &state).await?;
    assert_eq!(
        clipboard.0, "pending copy",
        "Repairing the saved rule must allow the pending copy to be transformed."
    );
    assert_eq!(
        history::get_all(&state.pool).await?.len(),
        1,
        "Retrying after a rule repair must record exactly one successful transformation."
    );
    state.pool.close().await;
    Ok(())
}

fn draft(name: &str) -> Rule {
    Rule::Trim(TrimRule {
        id: 0,
        name: name.into(),
        order: 0,
        is_active: true,
        trim_start: true,
        trim_end: true,
    })
}

#[tokio::test]
async fn migrations_defaults_and_persistence_survive_reopen() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join(constants::db::DB_FILE_NAME);
    let pool = open(&path).await?;
    assert!(
        rules::get_all(&pool).await?.is_empty(),
        "A newly created database must start with no saved rules."
    );
    assert_eq!(
        settings::get(&pool).await?,
        AppSettings::default(),
        "A newly created database must provide the documented default settings."
    );
    let saved = rules::save(&pool, draft("Keep me")).await?;
    let configuration = AppSettings {
        is_active: false,
        launch_at_startup: false,
        record_history: false,
    };
    settings::save(&pool, &configuration).await?;
    history::insert(&pool, " before ", "before").await?;
    pool.close().await;
    let reopened = open(&path).await?;
    assert_eq!(
        rules::get_all(&reopened).await?[0].id(),
        saved.id(),
        "Closing and reopening the database must preserve the saved rule's identity."
    );
    assert_eq!(
        settings::get(&reopened).await?,
        configuration,
        "Closing and reopening the database must preserve every saved setting."
    );
    assert_eq!(
        history::get_all(&reopened).await?.len(),
        1,
        "Closing and reopening the database must preserve recorded history."
    );
    reopened.close().await;
    Ok(())
}

#[tokio::test]
async fn create_update_reorder_delete_preserve_contiguous_positions() -> TestResult {
    let directory = tempfile::tempdir()?;
    let pool = open(&directory.path().join("test.db")).await?;
    let first = rules::save(&pool, draft("First")).await?;
    let second = rules::save(&pool, draft("Second")).await?;
    assert_ne!(
        first.id(),
        second.id(),
        "Each new rule must receive a distinct database ID."
    );
    assert_eq!(
        second.order(),
        1,
        "A second rule must append at position 1 because pipeline positions start at zero."
    );
    let reordered = rules::reorder(&pool, &[second.id(), first.id()]).await?;
    assert_eq!(
        reordered[0].id(),
        second.id(),
        "Reordering must place the requested rule first."
    );
    let mut edited = draft("Renamed");
    edited.set_identity(first.id(), 99);
    let edited = rules::save(&pool, edited).await?;
    assert_eq!(
        edited.name(),
        "Renamed",
        "Editing a saved rule must persist its new name."
    );
    assert_eq!(
        edited.order(),
        1,
        "Editing a rule must preserve its saved position and ignore the draft's arbitrary position."
    );
    rules::delete(&pool, second.id()).await?;
    let remaining = rules::get_all(&pool).await?;
    assert_eq!(
        remaining.len(),
        1,
        "Deleting one rule must leave the other rule intact."
    );
    assert_eq!(
        remaining[0].order(),
        0,
        "Deleting the first rule must move the remaining rule to position zero."
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn invalid_reorders_roll_back_without_changing_rules() -> TestResult {
    let directory = tempfile::tempdir()?;
    let pool = open(&directory.path().join("test.db")).await?;
    let first = rules::save(&pool, draft("First")).await?;
    let second = rules::save(&pool, draft("Second")).await?;
    for ids in [
        vec![],
        vec![first.id()],
        vec![first.id(), first.id()],
        vec![first.id(), 999],
    ] {
        assert!(
            matches!(
                rules::reorder(&pool, &ids).await,
                Err(AppError::Validation(_))
            ),
            "A reorder must contain every saved ID exactly once; rejected IDs: {ids:?}."
        );
        let existing = rules::get_all(&pool).await?;
        assert_eq!(
            existing.iter().map(Rule::id).collect::<Vec<_>>(),
            vec![first.id(), second.id()],
            "An invalid reorder must roll back completely and preserve the prior rule order; IDs: {ids:?}."
        );
    }
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn validation_and_missing_ids_do_not_persist_partial_changes() -> TestResult {
    let directory = tempfile::tempdir()?;
    let pool = open(&directory.path().join("test.db")).await?;
    assert!(
        matches!(
            rules::save(&pool, draft(" ")).await,
            Err(AppError::Validation(_))
        ),
        "A blank rule name must be rejected before saving."
    );
    let invalid = Rule::Regex(RegexRule {
        id: 0,
        name: "Bad".into(),
        order: 0,
        is_active: false,
        pattern: "(".into(),
        replacement: String::new(),
    });
    assert!(
        matches!(
            rules::save(&pool, invalid).await,
            Err(AppError::Validation(_))
        ),
        "An invalid regex must be rejected even when the rule is disabled."
    );
    let mut missing = draft("Missing");
    missing.set_identity(42, 0);
    assert!(
        matches!(
            rules::save(&pool, missing).await,
            Err(AppError::NotFound(_))
        ),
        "Editing an unknown rule ID must report that the rule does not exist."
    );
    assert!(
        matches!(rules::delete(&pool, 42).await, Err(AppError::NotFound(_))),
        "Deleting an unknown rule ID must report that the rule does not exist."
    );
    assert!(
        rules::get_all(&pool).await?.is_empty(),
        "Invalid saves and missing IDs must not leave partial rules in the database."
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn history_is_bounded_newest_first_and_clear_is_isolated() -> TestResult {
    let directory = tempfile::tempdir()?;
    let pool = open(&directory.path().join("test.db")).await?;
    rules::save(&pool, draft("Keep rule")).await?;
    let configuration = AppSettings {
        is_active: false,
        launch_at_startup: true,
        record_history: false,
    };
    settings::save(&pool, &configuration).await?;
    for index in 0..=constants::db::MAX_HISTORY_ENTRIES {
        history::insert(&pool, &index.to_string(), "after").await?;
    }
    let entries = history::get_all(&pool).await?;
    assert_eq!(
        i64::try_from(entries.len())?,
        constants::db::MAX_HISTORY_ENTRIES,
        "History must discard the oldest entries once the retention limit is reached."
    );
    assert_eq!(
        entries[0].original_text,
        constants::db::MAX_HISTORY_ENTRIES.to_string(),
        "History must be returned newest first."
    );
    assert!(
        entries[0].created_at.ends_with('Z'),
        "History timestamps must identify UTC with the Z suffix."
    );
    history::clear(&pool).await?;
    assert!(
        history::get_all(&pool).await?.is_empty(),
        "Clearing history must remove all retained transformations."
    );
    assert_eq!(
        rules::get_all(&pool).await?.len(),
        1,
        "Clearing history must preserve saved transformation rules."
    );
    assert_eq!(
        settings::get(&pool).await?,
        configuration,
        "Clearing history must preserve all saved settings."
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn pipeline_size_limit_is_enforced() -> TestResult {
    let directory = tempfile::tempdir()?;
    let pool = open(&directory.path().join("test.db")).await?;
    for _ in 0..constants::validation::MAX_RULES {
        rules::save(&pool, draft("Rule")).await?;
    }
    assert!(
        matches!(
            rules::save(&pool, draft("Too many")).await,
            Err(AppError::Validation(_))
        ),
        "Adding a rule beyond the pipeline limit must be rejected."
    );
    assert_eq!(
        rules::get_all(&pool).await?.len(),
        constants::validation::MAX_RULES,
        "A rejected extra rule must not increase the number of saved rules."
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn edited_rule_variants_and_pipeline_order_survive_reopen() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("test.db");
    let pool = open(&path).await?;
    let trim = rules::save(&pool, draft("Trim")).await?;
    let replacement = rules::save(
        &pool,
        Rule::Replace(ReplaceRule {
            id: 0,
            order: 0,
            name: "Literal".into(),
            is_active: false,
            target: "é\nvalue".into(),
            replacement: "$1".into(),
            match_case: false,
        }),
    )
    .await?;
    let mut regex = rules::save(
        &pool,
        Rule::Regex(RegexRule {
            id: 0,
            order: 0,
            name: "Regex".into(),
            is_active: true,
            pattern: "(?m)^(?<value>.+)$".into(),
            replacement: "${value}!".into(),
        }),
    )
    .await?;
    if let Rule::Regex(rule) = &mut regex {
        rule.replacement = "${value} edited".into();
        rule.is_active = false;
    }
    let regex_id = regex.id();
    rules::save(&pool, regex).await?;
    let expected = rules::reorder(&pool, &[replacement.id(), trim.id(), regex_id]).await?;
    pool.close().await;
    let reopened = open(&path).await?;
    assert_eq!(
        serde_json::to_value(rules::get_all(&reopened).await?)?,
        serde_json::to_value(expected)?,
        "All edited rule fields, enabled states, and pipeline positions must survive reopening the database."
    );
    reopened.close().await;
    Ok(())
}

#[tokio::test]
async fn failed_history_insert_reports_health_without_reprocessing_successful_write() -> TestResult
{
    use crate::{
        clipboard::{ClipboardAccess, Monitor},
        state::AppState,
    };
    struct TextClipboard {
        text: String,
        writes: usize,
    }
    impl ClipboardAccess for TextClipboard {
        fn read(&mut self) -> Result<Option<String>, AppError> {
            Ok(Some(self.text.clone()))
        }
        fn write(&mut self, text: &str) -> Result<(), AppError> {
            self.text = text.into();
            self.writes += 1;
            Ok(())
        }
    }
    let directory = tempfile::tempdir()?;
    let pool = open(&directory.path().join("test.db")).await?;
    rules::save(&pool, draft("Trim")).await?;
    sqlx::query("CREATE TRIGGER reject_history BEFORE INSERT ON history BEGIN SELECT RAISE(FAIL, 'history unavailable'); END")
        .execute(&pool).await?;
    let state = AppState::new(pool);
    let mut clipboard = TextClipboard {
        text: "baseline".into(),
        writes: 0,
    };
    let mut monitor = Monitor::default();
    monitor.tick(&mut clipboard, &state).await?;
    clipboard.text = " pending copy ".into();
    let tick = monitor.tick(&mut clipboard, &state).await?;
    assert_eq!(
        clipboard.text, "pending copy",
        "A successful clipboard write must remain applied even if recording history then fails."
    );
    assert!(
        tick.status.is_running,
        "A history-save failure must not stop clipboard monitoring."
    );
    assert!(
        tick.status
            .last_error
            .is_some_and(|error| error.contains("history unavailable")),
        "Worker health must explain that history could not be saved."
    );
    assert!(
        tick.change.is_some_and(|change| change.entry.is_none()),
        "A successful write with failed recording must report the change without a history entry."
    );
    assert!(
        history::get_all(&state.pool).await?.is_empty(),
        "A failed history insert must not leave a partial entry."
    );
    sqlx::query("DROP TRIGGER reject_history")
        .execute(&state.pool)
        .await?;
    let tick = monitor.tick(&mut clipboard, &state).await?;
    assert!(
        tick.status.last_error.is_none(),
        "After the history database is repaired, worker health must clear the old error."
    );
    assert!(
        tick.change.is_none(),
        "Repairing history must not reprocess the monitor's last successful clipboard write."
    );
    assert_eq!(
        clipboard.writes, 1,
        "A history-save failure must not cause duplicate writes of the same clipboard text."
    );
    clipboard.text = " next copy ".into();
    assert!(
        monitor
            .tick(&mut clipboard, &state)
            .await?
            .change
            .is_some_and(|change| change.entry.is_some()),
        "The next new copy after database repair must be transformed and recorded."
    );
    assert_eq!(
        history::get_all(&state.pool).await?.len(),
        1,
        "Only the new transformation after repair must appear in history."
    );
    state.pool.close().await;
    Ok(())
}
