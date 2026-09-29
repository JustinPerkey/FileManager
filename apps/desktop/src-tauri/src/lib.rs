#[cfg(test)]
mod generated_types;
mod tools;

pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init());
    tools::register(builder)
        .run(tauri::generate_context!())
        .expect("error while running FileManager");
}
