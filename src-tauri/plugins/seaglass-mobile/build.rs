// No command exposed to the UI: the app calls the Kotlin code from Rust only.
fn main() {
    tauri_plugin::Builder::new(&[])
        .android_path("android")
        .build();
}
