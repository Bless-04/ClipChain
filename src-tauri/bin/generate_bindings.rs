//! Standalone binary for regenerating TypeScript bindings via `tauri-specta`.
//!
//! # Usage
//!
//! ```bash
//! pnpm codegen
//! # or directly:
//! cargo run --bin generate_bindings --manifest-path src-tauri/Cargo.toml
//! ```
//!
//! The generated file (`src/lib/generated/bindings.ts`) is committed to Git
//! so the TypeScript compiler can run in CI without requiring the Rust toolchain.
//!
//! # Important
//!
//! Uses the runtime registry in `src-tauri/src/lib.rs`; add commands there.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The application and generator share exactly the same commands and events.
    clipchain_lib::ipc_builder().export(
        specta_typescript::Typescript::default()
            .bigint(specta_typescript::BigIntExportBehavior::Number),
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../src/lib/generated/bindings.ts"
        ),
    )?;
    Ok(())
}
