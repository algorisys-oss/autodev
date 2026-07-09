use std::path::Path;
use std::process::Command;

use crate::error::{AppError, AppResult};

/// Build a structured handoff prompt for an AI that drives a web browser. This is what an
/// agent (or the user) hands to a browser-automation tool to complete a web task.
pub fn build_handoff(task: &str, url: &str, context: &str) -> String {
    let task = task.trim();
    let url = url.trim();
    let context = context.trim();
    let start = if url.is_empty() {
        "Open the site relevant to the goal.".to_string()
    } else {
        url.to_string()
    };
    let context_section = if context.is_empty() {
        "None provided.".to_string()
    } else {
        context.to_string()
    };
    format!(
        "You are an AI assistant controlling a web browser. Complete this task end to end.\n\
         \n## Goal\n{task}\n\
         \n## Starting point\n{start}\n\
         \n## Context\n{context_section}\n\
         \n## How to proceed\n\
         - Navigate to the starting point.\n\
         - Take the steps needed to accomplish the goal, one at a time.\n\
         - If you must create an account or app, use sensible defaults and note what you chose.\n\
         - Do not change or delete anything unrelated to the goal.\n\
         \n## Report back\n\
         When done, report what you did, any credentials / IDs / URLs you created, and \
         anything that needs my input.\n"
    )
}

/// Run the configured browser command on a file containing the handoff prompt.
pub fn run_browser(template: &str, file: &Path) -> AppResult<String> {
    let quoted = format!("'{}'", file.display().to_string().replace('\'', "'\\''"));
    let rendered = template.replace("{file}", &quoted);
    let out = Command::new("sh")
        .arg("-c")
        .arg(&rendered)
        .output()
        .map_err(AppError::Io)?;
    if !out.status.success() {
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(AppError::Browser(if msg.is_empty() {
            "browser command failed".to_string()
        } else {
            msg
        }));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handoff_includes_all_sections_and_inputs() {
        let h = build_handoff(
            "Create a Discord application",
            "https://discord.com/developers",
            "Only Discord auth is allowed",
        );
        assert!(h.contains("## Goal\nCreate a Discord application"));
        assert!(h.contains("https://discord.com/developers"));
        assert!(h.contains("Only Discord auth is allowed"));
        assert!(h.contains("## Report back"));
    }

    #[test]
    fn handoff_uses_fallbacks_when_empty() {
        let h = build_handoff("do a thing", "", "");
        assert!(h.contains("Open the site relevant to the goal."));
        assert!(h.contains("None provided."));
    }

    #[test]
    fn run_browser_passes_the_handoff_file() {
        let file = std::env::temp_dir().join(format!("autodev-ho-{}.txt", std::process::id()));
        std::fs::write(&file, "HANDOFF-BODY").unwrap();
        let out = run_browser("cat {file}", &file).unwrap();
        assert_eq!(out, "HANDOFF-BODY");
        let _ = std::fs::remove_file(&file);
    }
}
