use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;

use crate::error::{AppError, AppResult};

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "'\\''"))
}

/// True if `bin` is found as a file on the `PATH` — a good-enough "is this tool installed"
/// check for picking a default screenshot command.
fn program_on_path(bin: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&paths).any(|dir| dir.join(bin).is_file())
}

/// A screenshot command to use when the user hasn't configured one: the first known tool
/// found on `PATH`, per platform, in preference order. `{file}` is the output PNG path;
/// capture is whole-screen (the annotate step lets the user crop). `None` if nothing is
/// installed, so the caller can prompt the user to install a tool or set a command.
pub fn default_screenshot_template() -> Option<String> {
    #[cfg(target_os = "macos")]
    let candidates: &[(&str, &str)] = &[("screencapture", "screencapture -x {file}")];
    #[cfg(target_os = "linux")]
    let candidates: &[(&str, &str)] = &[
        ("grim", "grim {file}"),
        ("spectacle", "spectacle -b -n -o {file}"),
        ("gnome-screenshot", "gnome-screenshot -f {file}"),
        ("scrot", "scrot {file}"),
        ("maim", "maim {file}"),
        ("import", "import -window root {file}"),
    ];
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let candidates: &[(&str, &str)] = &[];

    candidates
        .iter()
        .find(|(bin, _)| program_on_path(bin))
        .map(|(_, tmpl)| (*tmpl).to_string())
}

/// Run a screenshot command template (with `{file}` → a PNG path) and read the PNG bytes.
pub fn run_capture(template: &str, file: &Path) -> AppResult<Vec<u8>> {
    let rendered = template.replace("{file}", &shell_quote(file));
    let out = Command::new("sh")
        .arg("-c")
        .arg(&rendered)
        .output()
        .map_err(AppError::Io)?;
    if !out.status.success() {
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(AppError::Capture(if msg.is_empty() {
            "screenshot command failed".to_string()
        } else {
            msg
        }));
    }
    let bytes = fs::read(file)?;
    if bytes.is_empty() {
        return Err(AppError::Capture(
            "screenshot command produced no image".into(),
        ));
    }
    Ok(bytes)
}

/// Decode a base64 PNG and save it under `dir`, returning the file path. Used to persist
/// an annotated screenshot before attaching it to a prompt.
pub fn save_png(dir: &Path, data_b64: &str) -> AppResult<PathBuf> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data_b64.trim())
        .map_err(|e| AppError::Capture(format!("bad base64: {e}")))?;
    fs::create_dir_all(dir)?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let path = dir.join(format!("shot-{nonce}.png"));
    fs::write(&path, bytes)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_capture_reads_the_written_file() {
        // Template writes bytes to {file}; run_capture must read them back.
        let file = std::env::temp_dir().join(format!("autodev-cap-{}.png", std::process::id()));
        let bytes = run_capture("printf 'PNGDATA' > {file}", &file).unwrap();
        assert_eq!(&bytes, b"PNGDATA");
        let _ = fs::remove_file(&file);
    }

    #[test]
    fn run_capture_errors_when_no_file_produced() {
        let file = std::env::temp_dir().join("autodev-cap-missing.png");
        let _ = fs::remove_file(&file);
        assert!(run_capture("true", &file).is_err());
    }

    #[test]
    fn program_on_path_finds_a_real_tool_but_not_a_bogus_one() {
        assert!(program_on_path("sh"));
        assert!(!program_on_path("definitely-not-a-real-binary-xyz"));
    }

    #[test]
    fn default_screenshot_template_is_well_formed_when_present() {
        // Depends on what's installed; assert only the invariant: any default names {file}.
        if let Some(tmpl) = default_screenshot_template() {
            assert!(tmpl.contains("{file}"));
        }
    }

    #[test]
    fn save_png_decodes_and_writes() {
        let dir = std::env::temp_dir().join(format!("autodev-shots-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let b64 = base64::engine::general_purpose::STANDARD.encode(b"hello-png");
        let path = save_png(&dir, &b64).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"hello-png");
        let _ = fs::remove_dir_all(&dir);
    }
}
