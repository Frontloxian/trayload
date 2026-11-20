#[macro_use]
mod applog;
mod art;
mod backup;
mod collections;
mod installer;
mod launcher;
mod library;
mod music;
mod news;
mod setup;
mod registry;
mod secrets;

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
            applog::init(app.handle());
            let db = library::open(app.handle())?;
            secrets::migrate(&db, "steamgriddb", "steamgriddb_key");
            launcher::recover_sessions(&db)?;
            app.manage(db.clone());
            // Scan the ROM folders once at every launch, in the background.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(art::worker(handle.clone(), db.clone()));
            std::thread::spawn(move || {
                if let Err(err) = library::rescan_blocking(&handle, &db) {
                    crate::applog!("[scan] {err}");
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
            installer::check_updates,
            music::list_music_packs,
            music::get_track,
            music::open_music_folder,
            music::add_music_pack,
            music::remove_music_pack,
            backup::backup_preview,
            backup::backup_run,
            backup::backup_info,
            backup::restore_run,
            applog::get_app_info,
            applog::open_logs_folder,
            applog::log_frontend_error,
            library::list_removed_games,
            library::restore_game,
            news::get_news,
            news::open_news_link,
            setup::check_setup,
            setup::open_setup_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Trayload");
}
