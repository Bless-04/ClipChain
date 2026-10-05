fn main() {
    println!("cargo:rerun-if-changed=src/db/migrations");
    tauri_build::build()
}
