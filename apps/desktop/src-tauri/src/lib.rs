#[cfg(test)]
mod generated_types;
mod tools;

/// Starts the app and blocks until it exits.
///
/// # Panics
///
/// If Tauri cannot start, for example when the `WebView2` runtime is missing.
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init());
    tools::register(builder)
        .run(tauri::generate_context!())
        .expect("error while running FileManager");
}
