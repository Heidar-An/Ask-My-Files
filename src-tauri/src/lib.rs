mod commands;
mod extractors;
mod indexing;
mod models;
mod preview;
mod search;
mod semantic;
mod shell;
mod state;
mod storage;
mod utils;

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let state = AppState::new(app.handle())?;
            app.manage(state);
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::add_index_root,
            commands::get_file_details,
            commands::get_index_statuses,
            commands::list_index_roots,
            commands::open_file,
            commands::remove_index_root,
            commands::reveal_file,
            commands::search_files,
            commands::start_index
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
