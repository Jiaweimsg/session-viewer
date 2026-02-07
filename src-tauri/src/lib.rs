mod claude;
mod codex;
mod commands;
mod shared_models;
mod state;
mod watcher;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::get_projects,
            commands::get_sessions,
            commands::get_messages,
            commands::global_search,
            commands::get_stats,
            commands::get_token_summary,
            commands::resume_session,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            // Start file system watcher in background
            if let Err(e) = watcher::fs_watcher::start_watcher(handle) {
                eprintln!("Warning: Failed to start file watcher: {}", e);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
