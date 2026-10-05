//! Command handler modules.
//!
//! Each sub-module owns one domain's Tauri commands.
//! All commands must be:
//!   1. Decorated with `#[tauri::command]` and `#[specta::specta]`.
//!   2. Registered in `lib.rs` via the `tauri_specta` builder.
//!   3. Added to `collect_commands![]` in `bin/generate_bindings.rs`.

pub mod history;
pub mod rules;
pub mod settings;
