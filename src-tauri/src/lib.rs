mod agent;
mod agent_event;
mod approvals;
mod audio_record;
mod backend_spec;
mod capture;
mod commands;
mod editor;
mod error;
mod extensions;
mod git;
mod handoff;
pub mod headless;
mod loop_engine;
mod state;
mod task_split;
mod templates;
mod transcribe;
mod verify;
mod workspace;

use tauri::{Manager, WindowEvent};

/// On Linux, WebKitGTK disables the media-stream feature and denies `getUserMedia` by default,
/// so the voice recorder fails with a permission error. Enable media-stream and grant permission
/// requests on the main webview. Safe here because the webview only ever loads AutoDev's own
/// bundled UI — there is no untrusted remote page that could abuse mic/camera access.
#[cfg(target_os = "linux")]
fn allow_media_capture(app: &tauri::App) {
    use webkit2gtk::{PermissionRequestExt, SettingsExt, WebViewExt};
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let _ = window.with_webview(|platform| {
        let webview = platform.inner();
        if let Some(settings) = WebViewExt::settings(&webview) {
            settings.set_enable_media_stream(true);
        }
        webview.connect_permission_request(|_wv, req| {
            req.allow();
            true
        });
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(agent::AgentManager::default())
        .manage(audio_record::RecorderState::default())
        .setup(|app| {
            #[cfg(target_os = "linux")]
            allow_media_capture(app);
            Ok(())
        })
        .on_window_event(|window, event| {
            // Kill every live agent (and any in-progress mic capture) when the window closes, so
            // no PTY or ffmpeg child is orphaned.
            if let WindowEvent::CloseRequested { .. } = event {
                let app = window.app_handle();
                app.state::<agent::AgentManager>().kill_all();
                audio_record::kill(&app.state::<audio_record::RecorderState>());
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::backend_list,
            commands::list_templates,
            commands::skills_dir,
            commands::list_extensions,
            commands::get_settings,
            commands::set_settings,
            commands::open_in_editor,
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
            commands::respond_approval,
            commands::get_prompt_history,
            commands::add_prompt_history,
            commands::git_is_repo,
            commands::git_create_worktree,
            commands::git_worktree_status,
            commands::git_diff,
            commands::git_merge_worktree,
            commands::git_remove_worktree,
            commands::transcribe_audio,
            commands::record_start,
            commands::record_stop,
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
            commands::task_split_prompt,
            commands::task_split_parse,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
