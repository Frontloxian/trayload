mod installer;
mod registry;

#[tauri::command]
fn get_registry(app: tauri::AppHandle) -> Result<registry::Registry, String> {
    registry::load(&app)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(installer::Installing::default())
        .invoke_handler(tauri::generate_handler![
            get_registry,
            installer::list_emulators,
            installer::install_emulator,
            installer::launch_emulator,
            installer::uninstall_emulator,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Trayload");
}
