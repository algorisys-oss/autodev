mod agent;
mod capture;
mod commands;
mod error;
mod git;
mod handoff;
mod loop_engine;
mod state;
mod transcribe;
mod verify;
mod workspace;

use tauri::{Manager, WindowEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(agent::AgentManager::default())
        .on_window_event(|window, event| {
            // Kill every live agent when the window closes, so no PTY child is orphaned.
            if let WindowEvent::CloseRequested { .. } = event {
                window
                    .app_handle()
                    .state::<agent::AgentManager>()
                    .kill_all();
            }
        })
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
            commands::agent_spawn,
            commands::agent_write,
            commands::agent_resize,
            commands::agent_kill,
            commands::agent_list,
            commands::agent_kill_all,
            commands::get_prompt_history,
            commands::add_prompt_history,
            commands::git_is_repo,
            commands::git_create_worktree,
            commands::git_worktree_status,
            commands::git_diff,
            commands::git_merge_worktree,
            commands::git_remove_worktree,
            commands::transcribe_audio,
            commands::capture_screen,
            commands::save_shot,
            commands::generate_handoff,
            commands::run_browser_handoff,
            commands::loop_create,
            commands::loop_get,
            commands::loop_list,
            commands::loop_set_features,
            commands::loop_set_contract,
            commands::loop_ready_to_evaluate,
            commands::loop_grade,
            commands::loop_current_prompt,
            commands::loop_needs_compaction,
            commands::loop_compact_prompt,
            commands::loop_compact,
            commands::loop_apply_decomposer,
            commands::loop_apply_planner,
            commands::loop_apply_evaluator,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
