fn main() {
    // `sqlx::migrate!` embeds the files; a new one must rebuild the crate.
    println!("cargo:rerun-if-changed=migrations");
    tauri_build::build()
}
