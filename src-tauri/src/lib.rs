mod commands;
mod error;
mod state;
mod workspace;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::get_settings,
            commands::set_settings,
            commands::list_workspaces,
            commands::create_workspace,
            commands::delete_workspace,
            commands::add_project,
            commands::remove_project,
            commands::resolve_mention,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
