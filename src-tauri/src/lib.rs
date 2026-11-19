mod art;
mod collections;
mod installer;
mod launcher;
mod library;
mod registry;

use tauri::{Emitter, Manager};

#[tauri::command]
fn get_registry(app: tauri::AppHandle) -> Result<registry::Registry, String> {
    registry::load(&app)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(installer::Installing::default())
        .manage(library::LastScan::default())
        .manage(launcher::Running::default())
        .setup(|app| {
            let db = library::open(app.handle())?;
            launcher::recover_sessions(&db)?;
            app.manage(db.clone());
            // Scan the ROM folders once at every launch, in the background.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(art::worker(handle.clone(), db.clone()));
            std::thread::spawn(move || {
                if let Err(err) = library::rescan_blocking(&handle, &db) {
                    eprintln!("[scan] {err}");
                    let _ = handle.emit("scan-error", err);
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_registry,
            installer::list_emulators,
            installer::install_emulator,
            installer::launch_emulator,
            installer::uninstall_emulator,
            library::list_games,
            library::list_unassigned,
            library::last_scan,
            library::get_rom_folders,
            library::import_paths,
            library::add_rom_folder,
            library::remove_rom_folder,
            library::rescan,
            library::assign_system,
            library::dismiss_unassigned,
            library::remove_game,
            launcher::play_game,
            launcher::running_games,
            art::get_art_settings,
            art::set_steamgriddb_key,
            art::retry_art,
            art::set_art_from_file,
            art::get_cover,
            collections::set_favorite,
            collections::list_collections,
            collections::create_collection,
            collections::rename_collection,
            collections::delete_collection,
            collections::list_collection_members,
            collections::set_game_in_collection,
            collections::get_game_details,
            collections::save_overrides,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Trayload");
}
