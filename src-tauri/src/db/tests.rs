use super::{history, open, rules, settings};
use crate::{
    constants,
    error::AppError,
    models::{AppSettings, RegexRule, Rule, TrimRule},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

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
    assert!(monitor.tick(&mut clipboard, &state).await.is_err());
    assert_eq!(clipboard.0, "  pending copy  ");
    assert!(history::get_all(&state.pool).await?.is_empty());
    rules::save(&state.pool, saved).await?;
    monitor.tick(&mut clipboard, &state).await?;
    assert_eq!(clipboard.0, "pending copy");
    assert_eq!(history::get_all(&state.pool).await?.len(), 1);
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
    assert!(rules::get_all(&pool).await?.is_empty());
    assert_eq!(settings::get(&pool).await?, AppSettings::default());
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
    assert_eq!(rules::get_all(&reopened).await?[0].id(), saved.id());
    assert_eq!(settings::get(&reopened).await?, configuration);
    assert_eq!(history::get_all(&reopened).await?.len(), 1);
    reopened.close().await;
    Ok(())
}

#[tokio::test]
async fn create_update_reorder_delete_preserve_contiguous_positions() -> TestResult {
    let directory = tempfile::tempdir()?;
    let pool = open(&directory.path().join("test.db")).await?;
    let first = rules::save(&pool, draft("First")).await?;
    let second = rules::save(&pool, draft("Second")).await?;
    assert_ne!(first.id(), second.id());
    assert_eq!(second.order(), 1);
    let reordered = rules::reorder(&pool, &[second.id(), first.id()]).await?;
    assert_eq!(reordered[0].id(), second.id());
    let mut edited = draft("Renamed");
    edited.set_identity(first.id(), 99);
    let edited = rules::save(&pool, edited).await?;
    assert_eq!(edited.name(), "Renamed");
    assert_eq!(edited.order(), 1);
    rules::delete(&pool, second.id()).await?;
    let remaining = rules::get_all(&pool).await?;
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].order(), 0);
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
        assert!(matches!(
            rules::reorder(&pool, &ids).await,
            Err(AppError::Validation(_))
        ));
        let existing = rules::get_all(&pool).await?;
        assert_eq!(
            existing.iter().map(Rule::id).collect::<Vec<_>>(),
            vec![first.id(), second.id()]
        );
    }
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn validation_and_missing_ids_do_not_persist_partial_changes() -> TestResult {
    let directory = tempfile::tempdir()?;
    let pool = open(&directory.path().join("test.db")).await?;
    assert!(matches!(
        rules::save(&pool, draft(" ")).await,
        Err(AppError::Validation(_))
    ));
    let invalid = Rule::Regex(RegexRule {
        id: 0,
        name: "Bad".into(),
        order: 0,
        is_active: false,
        pattern: "(".into(),
        replacement: String::new(),
    });
    assert!(matches!(
        rules::save(&pool, invalid).await,
        Err(AppError::Validation(_))
    ));
    let mut missing = draft("Missing");
    missing.set_identity(42, 0);
    assert!(matches!(
        rules::save(&pool, missing).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        rules::delete(&pool, 42).await,
        Err(AppError::NotFound(_))
    ));
    assert!(rules::get_all(&pool).await?.is_empty());
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn history_is_bounded_newest_first_and_clear_is_isolated() -> TestResult {
    let directory = tempfile::tempdir()?;
    let pool = open(&directory.path().join("test.db")).await?;
    rules::save(&pool, draft("Keep rule")).await?;
    for index in 0..=constants::db::MAX_HISTORY_ENTRIES {
        history::insert(&pool, &index.to_string(), "after").await?;
    }
    let entries = history::get_all(&pool).await?;
    assert_eq!(
        i64::try_from(entries.len())?,
        constants::db::MAX_HISTORY_ENTRIES
    );
    assert_eq!(
        entries[0].original_text,
        constants::db::MAX_HISTORY_ENTRIES.to_string()
    );
    assert!(entries[0].created_at.ends_with('Z'));
    history::clear(&pool).await?;
    assert!(history::get_all(&pool).await?.is_empty());
    assert_eq!(rules::get_all(&pool).await?.len(), 1);
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
    assert!(matches!(
        rules::save(&pool, draft("Too many")).await,
        Err(AppError::Validation(_))
    ));
    assert_eq!(
        rules::get_all(&pool).await?.len(),
        constants::validation::MAX_RULES
    );
    pool.close().await;
    Ok(())
}
