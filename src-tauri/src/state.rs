use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

/// Persisted application settings.
///
/// This is the whole on-disk state for Phase 0. Later phases add sibling files
/// (workspaces, agent sessions) under the same data directory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// UI theme: "system" | "light" | "dark".
    pub theme: String,
    /// Default reasoning effort for new agents: "high" | "extra-high".
    pub default_effort: String,
    /// Shell command template used to transcribe a recording. `{file}` is replaced with
    /// the audio path. Empty/absent means voice input is not configured. Example:
    /// `whisper-cli -f {file} -otxt -of {file} && cat {file}.txt`.
    #[serde(default)]
    pub transcribe_command: Option<String>,
    /// Shell command template used to capture a screenshot to `{file}` (a PNG path).
    /// Example: `grim {file}` (Wayland), `scrot {file}` (X11), `screencapture {file}` (macOS).
    #[serde(default)]
    pub screenshot_command: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "system".to_string(),
            default_effort: "high".to_string(),
            transcribe_command: None,
            screenshot_command: None,
        }
    }
}

/// Root data directory for AutoDev state: `~/.autodev`.
pub fn data_dir() -> AppResult<PathBuf> {
    let home = dirs::home_dir().ok_or(AppError::NoHomeDir)?;
    Ok(home.join(".autodev"))
}

/// Directory for per-agent output logs: `~/.autodev/logs`.
pub fn logs_dir() -> AppResult<PathBuf> {
    Ok(data_dir()?.join("logs"))
}

/// Read settings from `dir/settings.json`, returning defaults if it is absent.
pub fn load_settings_from(dir: &Path) -> AppResult<AppSettings> {
    let path = dir.join("settings.json");
    match fs::read_to_string(&path) {
        Ok(raw) => Ok(serde_json::from_str(&raw)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(AppSettings::default()),
        Err(e) => Err(AppError::Io(e)),
    }
}

/// Write settings to `dir/settings.json`, creating `dir` if needed.
pub fn save_settings_to(dir: &Path, settings: &AppSettings) -> AppResult<()> {
    fs::create_dir_all(dir)?;
    let raw = serde_json::to_string_pretty(settings)?;
    fs::write(dir.join("settings.json"), raw)?;
    Ok(())
}

/// Load settings from the real data directory.
pub fn load_settings() -> AppResult<AppSettings> {
    load_settings_from(&data_dir()?)
}

/// Save settings to the real data directory.
pub fn save_settings(settings: &AppSettings) -> AppResult<()> {
    save_settings_to(&data_dir()?, settings)
}

const MAX_PROMPT_HISTORY: usize = 50;

/// Load saved prompts (newest first) from `dir/prompts.json`, empty if absent.
pub fn load_prompts_from(dir: &Path) -> AppResult<Vec<String>> {
    match fs::read_to_string(dir.join("prompts.json")) {
        Ok(raw) => Ok(serde_json::from_str(&raw)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(AppError::Io(e)),
    }
}

/// Prepend `text` to the prompt history (de-duplicated, capped) and persist it.
pub fn add_prompt_to(dir: &Path, text: &str) -> AppResult<Vec<String>> {
    let text = text.trim();
    let mut prompts = load_prompts_from(dir)?;
    prompts.retain(|p| p != text);
    if !text.is_empty() {
        prompts.insert(0, text.to_string());
    }
    prompts.truncate(MAX_PROMPT_HISTORY);
    fs::create_dir_all(dir)?;
    fs::write(
        dir.join("prompts.json"),
        serde_json::to_string_pretty(&prompts)?,
    )?;
    Ok(prompts)
}

/// Load prompt history from the real data directory.
pub fn load_prompts() -> AppResult<Vec<String>> {
    load_prompts_from(&data_dir()?)
}

/// Add a prompt to the history in the real data directory.
pub fn add_prompt(text: &str) -> AppResult<Vec<String>> {
    add_prompt_to(&data_dir()?, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "autodev-test-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn load_returns_defaults_when_missing() {
        let dir = temp_dir("defaults");
        let s = load_settings_from(&dir).unwrap();
        assert_eq!(s, AppSettings::default());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = temp_dir("roundtrip");
        let custom = AppSettings {
            theme: "dark".to_string(),
            default_effort: "extra-high".to_string(),
            transcribe_command: Some("whisper {file}".to_string()),
            screenshot_command: Some("grim {file}".to_string()),
        };
        save_settings_to(&dir, &custom).unwrap();
        let loaded = load_settings_from(&dir).unwrap();
        assert_eq!(loaded, custom);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn prompt_history_dedupes_and_orders_newest_first() {
        let dir = temp_dir("prompts");
        add_prompt_to(&dir, "first").unwrap();
        add_prompt_to(&dir, "second").unwrap();
        let after = add_prompt_to(&dir, "first").unwrap(); // re-adding moves it to front
        assert_eq!(after, vec!["first", "second"]);
        assert_eq!(load_prompts_from(&dir).unwrap(), vec!["first", "second"]);
        let _ = fs::remove_dir_all(&dir);
    }
}
