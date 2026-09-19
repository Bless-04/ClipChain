//! `ClipChain` library crate root.
//!
//! This file is responsible for:
//! 1. Declaring all Rust modules.
//! 2. Enforcing project-wide lint rules (per AGENTS.md §4).
//! 3. Constructing and running the Tauri application.
//!
//! `ipc_builder()` is the shared runtime and binding-generation registry.

// ---------------------------------------------------------------------------
// Project-wide lint enforcement (AGENTS.md §4)
// ---------------------------------------------------------------------------

// Exceptions require a narrow `#[allow(...)]` with an inline explanation.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::pedantic
)]
#![warn(missing_docs, unreachable_pub)]

// ---------------------------------------------------------------------------
// Module declarations
// ---------------------------------------------------------------------------

pub mod clipboard;
pub mod commands;
pub mod constants;
pub mod db;
pub mod error;
pub mod models;
pub mod pipeline;
pub mod state;

// ---------------------------------------------------------------------------
// App bootstrap
// ---------------------------------------------------------------------------

use models::events::{
    ClipboardChanged, HistoryCleared, MonitorChanged, RulesUpdated, SettingsUpdated,
};
use std::sync::{atomic::Ordering, Arc};
use tauri::Manager;
use tauri_specta::{collect_commands, collect_events, Builder, Event};

/// Builds the single registry used by both the application and code generation.
#[must_use]
pub fn ipc_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            commands::rules::get_rules,
            commands::rules::save_rule,
            commands::rules::delete_rule,
            commands::rules::reorder_rules,
            commands::rules::preview_rule,
            commands::history::get_history,
            commands::history::clear_history,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::get_monitor_status,
        ])
        .events(collect_events![
            ClipboardChanged,
            RulesUpdated,
            SettingsUpdated,
            HistoryCleared,
            MonitorChanged
        ])
        .typ::<models::RuleType>()
        .constant("RULE_TRIM", models::RuleType::Trim)
        .constant("RULE_REPLACE", models::RuleType::Replace)
        .constant("RULE_REGEX", models::RuleType::Regex)
        .constant("CLIPBOARD_CHANGED_EVENT", ClipboardChanged::NAME)
        .constant("RULES_UPDATED_EVENT", RulesUpdated::NAME)
        .constant("SETTINGS_UPDATED_EVENT", SettingsUpdated::NAME)
        .constant("HISTORY_CLEARED_EVENT", HistoryCleared::NAME)
        .constant("MONITOR_CHANGED_EVENT", MonitorChanged::NAME)
        .constant("MAX_RULES", constants::validation::MAX_RULES)
        .constant("MAX_NAME_CHARS", constants::validation::MAX_NAME_CHARS)
        .constant("MAX_FIELD_BYTES", constants::validation::MAX_FIELD_BYTES)
        .constant("MAX_TEXT_BYTES", constants::clipboard::MAX_TEXT_BYTES)
        .constant("MAX_HISTORY_ENTRIES", constants::db::MAX_HISTORY_ENTRIES)
}

/// Initializes the database, plugins and worker, and runs the desktop application.
/// # Errors
/// Returns startup errors rather than panicking on configuration or database failures.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let specta_builder = ipc_builder();
    let application = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window(constants::app::MAIN_WINDOW) {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app| {
            specta_builder.mount_events(app);
            let directory = app.path().app_data_dir()?;
            std::fs::create_dir_all(&directory)?;
            let pool = tauri::async_runtime::block_on(db::open(
                &directory.join(constants::db::DB_FILE_NAME),
            ))?;
            let state = Arc::new(state::AppState::new(pool));
            app.manage(state.clone());
            clipboard::start(app.handle().clone(), state)?;
            Ok(())
        })
        .build(tauri::generate_context!())?;
    application.run(|app, event| {
        if let tauri::RunEvent::Exit = event {
            app.state::<Arc<state::AppState>>()
                .shutdown
                .store(true, Ordering::Relaxed);
        }
    });
    Ok(())
}
