//! Rust-owned runtime state. `SQLite` remains the source of persisted domain data.

use crate::{
    db,
    error::AppError,
    models::{events::MonitorStatus, AppSettings},
};
use sqlx::SqlitePool;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{Mutex, RwLock};

/// Shared database, mutation gate, and clipboard worker lifecycle.
pub struct AppState {
    pub(crate) pool: SqlitePool,
    // Serialize configuration mutations and clipboard writes so pause/save/clear
    // cannot race an in-flight transformation or history insert.
    pub(crate) gate: Mutex<()>,
    pub(crate) monitor: RwLock<MonitorStatus>,
    pub(crate) shutdown: AtomicBool,
    pub(crate) reset_baseline: AtomicBool,
}

impl AppState {
    /// Creates managed state for an initialized database.
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            gate: Mutex::new(()),
            monitor: RwLock::new(MonitorStatus::default()),
            shutdown: AtomicBool::new(false),
            reset_baseline: AtomicBool::new(false),
        }
    }

    /// Persists settings and remembers pauses even between worker polls.
    /// The caller holds the mutation gate, including native startup coordination.
    pub(crate) async fn persist_settings(&self, settings: &AppSettings) -> Result<(), AppError> {
        db::settings::save(&self.pool, settings).await?;
        if !settings.is_active {
            self.reset_baseline.store(true, Ordering::Relaxed);
        }
        Ok(())
    }
}
