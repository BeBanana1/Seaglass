// Aucune commande exposée à l'UI : l'app appelle le code Kotlin depuis Rust seulement.
fn main() {
    tauri_plugin::Builder::new(&[])
        .android_path("android")
        .build();
}
