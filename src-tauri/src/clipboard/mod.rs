//! Native clipboard worker and independently testable polling state machine.

use crate::{
    constants::clipboard,
    db,
    error::AppError,
    models::events::{ClipboardChanged, MonitorChanged, MonitorStatus},
    pipeline::service::run_pipeline,
    state::AppState,
};
use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};
use tauri_specta::Event;

pub(crate) trait ClipboardAccess {
    fn read(&mut self) -> Result<Option<String>, AppError>;
    fn write(&mut self, text: &str) -> Result<(), AppError>;
}

impl ClipboardAccess for arboard::Clipboard {
    fn read(&mut self) -> Result<Option<String>, AppError> {
        match self.get_text() {
            Ok(text) => Ok(Some(text)),
            Err(arboard::Error::ContentNotAvailable) => Ok(None),
            Err(error) => Err(AppError::Clipboard(error.to_string())),
        }
    }
    fn write(&mut self, text: &str) -> Result<(), AppError> {
        self.set_text(text)
            .map_err(|error| AppError::Clipboard(error.to_string()))
    }
}

#[derive(Default)]
pub(crate) struct Monitor {
    initialized: bool,
    last_text: Option<String>,
}

pub(crate) struct Tick {
    pub(crate) status: MonitorStatus,
    pub(crate) change: Option<ClipboardChanged>,
}

impl Monitor {
    pub(crate) async fn tick(
        &mut self,
        clipboard: &mut impl ClipboardAccess,
        state: &AppState,
    ) -> Result<Tick, AppError> {
        let _guard = state.gate.lock().await;
        let settings = db::settings::get(&state.pool).await?;
        if state.reset_baseline.swap(false, Ordering::Relaxed) {
            self.initialized = false;
            self.last_text = None;
        }
        let mut tick = Tick {
            status: MonitorStatus {
                is_running: settings.is_active,
                last_error: None,
            },
            change: None,
        };
        if !settings.is_active {
            // Paused means no clipboard reads, and resume establishes a fresh baseline.
            self.initialized = false;
            self.last_text = None;
            return Ok(tick);
        }
        let current = clipboard.read()?;
        if !self.initialized {
            self.initialized = true;
            self.last_text = current;
            return Ok(tick);
        }
        if current == self.last_text {
            return Ok(tick);
        }
        let Some(original) = current else {
            self.last_text = None;
            return Ok(tick);
        };
        if original.len() > clipboard::MAX_TEXT_BYTES {
            self.last_text = Some(original);
            return Err(AppError::Validation(
                "Clipboard text exceeds the 1 MiB limit and was skipped".into(),
            ));
        }
        let rules = db::rules::get_all(&state.pool).await?;
        let transformed = match run_pipeline(&rules, &original) {
            Ok(transformed) => transformed,
            Err(error) => {
                // Deterministically invalid input is skipped; transient database
                // and clipboard read failures leave the pending copy retryable.
                self.last_text = Some(original);
                return Err(error);
            }
        };
        if original == transformed {
            self.last_text = Some(original);
            return Ok(tick);
        }
        // Do not replace a newer copy made while the pipeline was running.
        // This is a best-effort cross-platform guard, not an OS compare-and-swap.
        if clipboard.read()?.as_deref() != Some(original.as_str()) {
            self.last_text = Some(original);
            return Ok(tick);
        }
        if let Err(error) = clipboard.write(&transformed) {
            self.last_text = None; // A temporarily locked clipboard can be retried.
            return Err(error);
        }
        self.last_text = Some(transformed.clone()); // Suppress our own write on the next poll.
        let entry = if settings.record_history {
            match db::history::insert(&state.pool, &original, &transformed).await {
                Ok(entry) => Some(entry),
                Err(error) => {
                    tick.status.last_error = Some(format!(
                        "Clipboard transformed, but history could not be saved: {error}"
                    ));
                    None
                }
            }
        } else {
            None
        };
        tick.change = Some(ClipboardChanged { entry });
        Ok(tick)
    }
}

async fn report(app: &tauri::AppHandle, state: &AppState, status: MonitorStatus) {
    let mut previous = state.monitor.write().await;
    if *previous != status {
        *previous = status.clone();
        let _ = MonitorChanged(status).emit(app);
    }
}

/// Starts one persistent clipboard owner thread. It stops when the app exits.
/// # Errors
/// Returns an internal error if the OS cannot create the worker thread.
pub fn start(app: tauri::AppHandle, state: Arc<AppState>) -> Result<(), AppError> {
    std::thread::Builder::new()
        .name(clipboard::WORKER_NAME.into())
        .spawn(move || {
            let mut device = None;
            let mut monitor = Monitor::default();
            while !state.shutdown.load(Ordering::Relaxed) {
                let result = match device.as_mut() {
                    Some(clipboard) => {
                        tauri::async_runtime::block_on(monitor.tick(clipboard, &state))
                    }
                    None => match arboard::Clipboard::new() {
                        Ok(clipboard) => {
                            device = Some(clipboard);
                            continue;
                        }
                        Err(error) => Err(AppError::Clipboard(error.to_string())),
                    },
                };
                tauri::async_runtime::block_on(async {
                    match result {
                        Ok(tick) => {
                            report(&app, &state, tick.status).await;
                            if let Some(change) = tick.change {
                                let _ = change.emit(&app);
                            }
                        }
                        Err(error) => {
                            report(
                                &app,
                                &state,
                                MonitorStatus {
                                    is_running: false,
                                    last_error: Some(error.to_string()),
                                },
                            )
                            .await;
                        }
                    }
                });
                std::thread::sleep(Duration::from_millis(clipboard::POLL_INTERVAL_MS));
            }
        })
        .map_err(|error| AppError::Internal(error.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod native_tests;
