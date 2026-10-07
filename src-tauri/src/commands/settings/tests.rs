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
        assert_eq!(
            *changes.borrow(),
            vec![!previous],
            "Changing the startup preference must call the OS operation once with the new value; previous value: {previous}."
        );
        assert_eq!(
            db::settings::get(&state.pool).await?,
            settings,
            "A successful startup change must persist every requested setting."
        );
        assert!(
            state.reset_baseline.load(Ordering::Relaxed),
            "Pausing through settings must reset the clipboard starting point before a later resume."
        );
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
        assert_eq!(
            db::settings::get(&state.pool).await?,
            settings,
            "Other settings must still save when startup registration is unchanged."
        );
        assert!(
            !state.reset_baseline.load(Ordering::Relaxed),
            "Changing recording alone must not reset the clipboard starting point."
        );
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
        matches!(result, Err(AppError::Internal(message)) if message.contains("Could not change startup registration") && message.contains("Registration denied")),
        "A rejected OS startup change must explain both the failed operation and the OS error."
    );
    assert_eq!(
        *changes.borrow(),
        vec![true],
        "A rejected startup change must not trigger extra registration attempts."
    );
    assert_eq!(
        db::settings::get(&state.pool).await?,
        AppSettings::default(),
        "If startup registration fails, all saved settings must remain at their previous values."
    );
    assert!(
        !state.reset_baseline.load(Ordering::Relaxed),
        "A rejected settings change must not reset the monitor's starting point."
    );
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
        assert!(
            matches!(result, Err(AppError::Database(_))),
            "A failed settings save must still report the database error after restoring OS registration."
        );
        assert_eq!(
            *changes.borrow(),
            vec![!previous, previous],
            "If saving settings fails, restore the prior OS startup value; previous value: {previous}."
        );
        assert!(
            !state.reset_baseline.load(Ordering::Relaxed),
            "Settings that failed to save must not reset the monitor's starting point."
        );
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
        matches!(result, Err(AppError::Internal(message)) if message.contains("Database error") && message.contains("startup rollback also failed") && message.contains("Rollback denied")),
        "If saving and restoring startup both fail, the error must explain both failures."
    );
    assert_eq!(
        *changes.borrow(),
        vec![true, false],
        "A rollback failure must still show that restoring the previous startup value was attempted."
    );
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
    assert!(
        matches!(result, Err(AppError::Database(_))),
        "A database failure must be reported even when no startup change was needed."
    );
    assert!(
        changes.borrow().is_empty(),
        "Do not attempt OS rollback when this operation never changed startup registration."
    );
    Ok(())
}
