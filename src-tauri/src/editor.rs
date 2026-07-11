use std::path::Path;

use crate::error::{AppError, AppResult};

/// Build the `(program, args)` to open `path` in `editor`.
///
/// `editor` is a simple command line (e.g. `code` or `code -n`) split on whitespace and
/// **never** run through a shell — the canonicalized `path` is appended as the final argument,
/// so a path can never be interpreted as a shell fragment (LOOPS XV). Errors if the editor
/// command is empty or the path does not exist (typo / stale-worktree guard).
pub fn build_open_command(editor: &str, path: &str) -> AppResult<(String, Vec<String>)> {
    let mut parts = editor.split_whitespace().map(String::from);
    let program = parts
        .next()
        .ok_or_else(|| AppError::NotFound("editor command (set one in Settings)".into()))?;
    let canonical = std::fs::canonicalize(Path::new(path))
        .map_err(|_| AppError::NotFound(format!("path {path}")))?;
    let mut args: Vec<String> = parts.collect();
    args.push(canonical.to_string_lossy().to_string());
    Ok((program, args))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_program_and_appends_existing_path() {
        let dir = std::env::temp_dir();
        let (program, args) = build_open_command("code", &dir.to_string_lossy()).unwrap();
        assert_eq!(program, "code");
        assert_eq!(args.len(), 1);
        assert!(Path::new(&args[0]).exists());
    }

    #[test]
    fn keeps_extra_args_before_the_path() {
        let dir = std::env::temp_dir();
        let (program, args) = build_open_command("code -n", &dir.to_string_lossy()).unwrap();
        assert_eq!(program, "code");
        assert_eq!(args[0], "-n");
        assert_eq!(args.len(), 2);
        assert!(Path::new(&args[1]).exists());
    }

    #[test]
    fn empty_editor_command_errors() {
        let dir = std::env::temp_dir();
        assert!(build_open_command("   ", &dir.to_string_lossy()).is_err());
    }

    #[test]
    fn missing_path_errors() {
        assert!(build_open_command("code", "/no/such/path-autodev-xyz-123").is_err());
    }
}
