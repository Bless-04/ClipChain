use super::persist_with_startup;
use crate::{db, error::AppError, models::AppSettings, state::AppState};
use std::{cell::RefCell, sync::atomic::Ordering};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[tokio::test]
async fn startup_enable_and_disable_persist_all_settings() -> TestResult {
    for previous in [false, true] {
        let directory = tempfile::tempdir()?;
        let state = AppState::new(db::open(&directory.path().join("test.db")).await?);
        let settings = AppSettings {
            is_active: false,
            launch_at_startup: !previous,
            record_history: false,
        };
        let changes = RefCell::new(Vec::new());
        persist_with_startup(&state, &settings, previous, |enabled| {
            changes.borrow_mut().push(enabled);
            Ok(())
        })
        .await?;
        assert_eq!(*changes.borrow(), vec![!previous]);
        assert_eq!(db::settings::get(&state.pool).await?, settings);
        assert!(state.reset_baseline.load(Ordering::Relaxed));
        state.pool.close().await;
    }
    Ok(())
}

#[tokio::test]
async fn unchanged_startup_registration_is_not_touched() -> TestResult {
    for previous in [false, true] {
        let directory = tempfile::tempdir()?;
        let state = AppState::new(db::open(&directory.path().join("test.db")).await?);
        let settings = AppSettings {
            launch_at_startup: previous,
            record_history: false,
            ..Default::default()
        };
        persist_with_startup(&state, &settings, previous, |_| {
            Err("Startup must not be touched".into())
        })
        .await?;
        assert_eq!(db::settings::get(&state.pool).await?, settings);
        assert!(!state.reset_baseline.load(Ordering::Relaxed));
        state.pool.close().await;
    }
    Ok(())
}

#[tokio::test]
async fn startup_failure_preserves_saved_settings_and_pause_baseline() -> TestResult {
    let directory = tempfile::tempdir()?;
    let state = AppState::new(db::open(&directory.path().join("test.db")).await?);
    let settings = AppSettings {
        is_active: false,
        launch_at_startup: true,
        record_history: false,
    };
    let changes = RefCell::new(Vec::new());
    let result = persist_with_startup(&state, &settings, false, |enabled| {
        changes.borrow_mut().push(enabled);
        Err("Registration denied".into())
    })
    .await;
    assert!(
        matches!(result, Err(AppError::Internal(message)) if message.contains("Could not change startup registration") && message.contains("Registration denied"))
    );
    assert_eq!(*changes.borrow(), vec![true]);
    assert_eq!(
        db::settings::get(&state.pool).await?,
        AppSettings::default()
    );
    assert!(!state.reset_baseline.load(Ordering::Relaxed));
    state.pool.close().await;
    Ok(())
}

#[tokio::test]
async fn database_failure_restores_previous_startup_registration() -> TestResult {
    for previous in [false, true] {
        let directory = tempfile::tempdir()?;
        let state = AppState::new(db::open(&directory.path().join("test.db")).await?);
        state.pool.close().await;
        let settings = AppSettings {
            is_active: false,
            launch_at_startup: !previous,
            ..Default::default()
        };
        let changes = RefCell::new(Vec::new());
        let result = persist_with_startup(&state, &settings, previous, |enabled| {
            changes.borrow_mut().push(enabled);
            Ok(())
        })
        .await;
        assert!(matches!(result, Err(AppError::Database(_))));
        assert_eq!(*changes.borrow(), vec![!previous, previous]);
        assert!(!state.reset_baseline.load(Ordering::Relaxed));
    }
    Ok(())
}

#[tokio::test]
async fn rollback_failure_reports_both_failures() -> TestResult {
    let directory = tempfile::tempdir()?;
    let state = AppState::new(db::open(&directory.path().join("test.db")).await?);
    state.pool.close().await;
    let settings = AppSettings {
        launch_at_startup: true,
        ..Default::default()
    };
    let changes = RefCell::new(Vec::new());
    let result = persist_with_startup(&state, &settings, false, |enabled| {
        changes.borrow_mut().push(enabled);
        if enabled {
            Ok(())
        } else {
            Err("Rollback denied".into())
        }
    })
    .await;
    assert!(
        matches!(result, Err(AppError::Internal(message)) if message.contains("Database error") && message.contains("startup rollback also failed") && message.contains("Rollback denied"))
    );
    assert_eq!(*changes.borrow(), vec![true, false]);
    Ok(())
}

#[tokio::test]
async fn database_failure_without_startup_change_does_not_attempt_rollback() -> TestResult {
    let directory = tempfile::tempdir()?;
    let state = AppState::new(db::open(&directory.path().join("test.db")).await?);
    state.pool.close().await;
    let changes = RefCell::new(Vec::new());
    let result = persist_with_startup(&state, &AppSettings::default(), false, |enabled| {
        changes.borrow_mut().push(enabled);
        Ok(())
    })
    .await;
    assert!(matches!(result, Err(AppError::Database(_))));
    assert!(changes.borrow().is_empty());
    Ok(())
}
