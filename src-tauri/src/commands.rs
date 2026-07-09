use serde::Serialize;

use crate::error::AppResult;
use crate::state::{self, AppSettings};

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
