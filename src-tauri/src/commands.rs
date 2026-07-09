use serde::Serialize;

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
