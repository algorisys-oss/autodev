use std::io::Write;
use std::sync::{Arc, Mutex};

use base64::Engine;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::agent::{AgentInfo, AgentManager, AgentOptions};
use crate::error::AppResult;
use crate::state::{self, AppSettings};
use crate::workspace::{self, ResolvedMention, Workspace, WorkspaceStore};

/// Load the workspace store from the real data directory.
fn load_store() -> AppResult<WorkspaceStore> {
    workspace::load_store_from(&state::data_dir()?)
}

/// Save the workspace store to the real data directory.
fn save_store(store: &WorkspaceStore) -> AppResult<()> {
    workspace::save_store_to(&state::data_dir()?, store)
}

/// Static info about the running app. Used by the frontend on startup to prove
/// the command+event bridge is wired and to show the version.
#[derive(Debug, Serialize)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        name: "AutoDev".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

#[tauri::command]
pub fn get_settings() -> AppResult<AppSettings> {
    state::load_settings()
}

#[tauri::command]
pub fn set_settings(settings: AppSettings) -> AppResult<AppSettings> {
    state::save_settings(&settings)?;
    Ok(settings)
}

#[tauri::command]
pub fn list_workspaces() -> AppResult<Vec<Workspace>> {
    Ok(load_store()?.workspaces)
}

#[tauri::command]
pub fn create_workspace(name: String) -> AppResult<Workspace> {
    let mut store = load_store()?;
    let ws = store.create_workspace(&name);
    save_store(&store)?;
    Ok(ws)
}

#[tauri::command]
pub fn delete_workspace(id: String) -> AppResult<()> {
    let mut store = load_store()?;
    store.delete_workspace(&id)?;
    save_store(&store)
}

#[tauri::command]
pub fn add_project(workspace_id: String, path: String) -> AppResult<Workspace> {
    let mut store = load_store()?;
    let ws = store.add_project(&workspace_id, &path)?;
    save_store(&store)?;
    Ok(ws)
}

#[tauri::command]
pub fn remove_project(workspace_id: String, project_name: String) -> AppResult<Workspace> {
    let mut store = load_store()?;
    let ws = store.remove_project(&workspace_id, &project_name)?;
    save_store(&store)?;
    Ok(ws)
}

#[tauri::command]
pub fn resolve_mention(workspace_id: String, token: String) -> AppResult<Option<ResolvedMention>> {
    let store = load_store()?;
    let ws = store
        .workspaces
        .iter()
        .find(|w| w.id == workspace_id)
        .ok_or_else(|| crate::error::AppError::NotFound(format!("workspace {workspace_id}")))?;
    Ok(workspace::resolve_mention(ws, &token))
}

// --- Agents (Phase 2) ---

#[derive(Clone, Serialize)]
struct OutputEvent {
    id: String,
    /// base64 of the raw PTY bytes, so terminal escape sequences survive intact.
    data: String,
}

#[derive(Clone, Serialize)]
struct ExitEvent {
    id: String,
    code: Option<i32>,
}

#[tauri::command]
pub fn agent_spawn(
    app: AppHandle,
    manager: State<'_, AgentManager>,
    options: AgentOptions,
    cols: Option<u16>,
    rows: Option<u16>,
) -> AppResult<String> {
    let id = manager.next_id();
    let cols = cols.unwrap_or(80);
    let rows = rows.unwrap_or(24);

    // Append raw output to a per-agent log on disk, so scrollback survives a crash
    // and the run is auditable (LOOPS XXX). Best-effort: logging never blocks a spawn.
    let log: Option<Arc<Mutex<std::fs::File>>> = state::logs_dir().ok().and_then(|dir| {
        std::fs::create_dir_all(&dir).ok()?;
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join(format!("{id}.log")))
            .ok()
            .map(|f| Arc::new(Mutex::new(f)))
    });

    let out_app = app.clone();
    let out_id = id.clone();
    let on_output = move |bytes: Vec<u8>| {
        if let Some(log) = &log {
            if let Ok(mut f) = log.lock() {
                let _ = f.write_all(&bytes);
            }
        }
        let data = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let _ = out_app.emit(
            "agent://output",
            OutputEvent {
                id: out_id.clone(),
                data,
            },
        );
    };

    let exit_app = app.clone();
    let exit_id = id.clone();
    let on_exit = move |code: Option<i32>| {
        let _ = exit_app.emit(
            "agent://exit",
            ExitEvent {
                id: exit_id.clone(),
                code,
            },
        );
    };

    let session =
        crate::agent::spawn_session(id.clone(), &options, cols, rows, on_output, on_exit)?;
    manager.insert(session);
    Ok(id)
}

#[tauri::command]
pub fn agent_write(manager: State<'_, AgentManager>, id: String, data: String) -> AppResult<()> {
    manager.get(&id)?.write(data.as_bytes())
}

#[tauri::command]
pub fn agent_resize(
    manager: State<'_, AgentManager>,
    id: String,
    cols: u16,
    rows: u16,
) -> AppResult<()> {
    manager.get(&id)?.resize(cols, rows)
}

#[tauri::command]
pub fn agent_kill(manager: State<'_, AgentManager>, id: String) -> AppResult<()> {
    let session = manager.get(&id)?;
    session.kill()?;
    Ok(())
}

#[tauri::command]
pub fn agent_list(manager: State<'_, AgentManager>) -> Vec<AgentInfo> {
    manager.list()
}

#[tauri::command]
pub fn agent_kill_all(manager: State<'_, AgentManager>) -> usize {
    manager.kill_all()
}

// --- Prompt history (Phase 4) ---

#[tauri::command]
pub fn get_prompt_history() -> AppResult<Vec<String>> {
    state::load_prompts()
}

#[tauri::command]
pub fn add_prompt_history(text: String) -> AppResult<Vec<String>> {
    state::add_prompt(&text)
}

// --- Git worktrees (Phase 5) ---

use crate::git::{self, WorktreeStatus};
use std::path::{Path, PathBuf};

/// A worktree created for isolated agent work.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeInfo {
    pub repo: String,
    pub path: String,
    pub branch: String,
}

#[tauri::command]
pub fn git_is_repo(dir: String) -> bool {
    git::is_repo(Path::new(&dir))
}

/// Turn a branch name into a directory-safe slug for the worktree folder.
fn worktree_slug(branch: &str) -> String {
    branch
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect()
}

#[tauri::command]
pub fn git_create_worktree(repo: String, branch: String) -> AppResult<WorktreeInfo> {
    let repo_path = PathBuf::from(&repo);
    let path = state::data_dir()?
        .join("worktrees")
        .join(worktree_slug(&branch));
    if path.exists() {
        return Err(crate::error::AppError::Conflict(format!(
            "worktree path already exists: {}",
            path.display()
        )));
    }
    std::fs::create_dir_all(path.parent().unwrap())?;
    git::create_worktree(&repo_path, &path, &branch)?;
    Ok(WorktreeInfo {
        repo,
        path: path.to_string_lossy().to_string(),
        branch,
    })
}

#[tauri::command]
pub fn git_worktree_status(path: String) -> AppResult<WorktreeStatus> {
    git::status(Path::new(&path))
}

#[tauri::command]
pub fn git_diff(repo: String, branch: String) -> AppResult<String> {
    git::diff(Path::new(&repo), &branch)
}

#[tauri::command]
pub fn git_merge_worktree(repo: String, branch: String) -> AppResult<String> {
    git::merge(Path::new(&repo), &branch)
}

#[tauri::command]
pub fn git_remove_worktree(repo: String, path: String, force: bool) -> AppResult<()> {
    git::remove_worktree(Path::new(&repo), Path::new(&path), force)
}
