#![allow(clippy::expect_used)]

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
