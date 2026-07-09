use std::path::Path;
use std::process::Command;

use crate::error::{AppError, AppResult};

/// Render a transcription command template by substituting `{file}` with the
/// (shell-quoted) audio path. The result is run through `sh -c`, so the template can be
/// any pipeline, e.g. `whisper-cli -f {file} -otxt -of {file} && cat {file}.txt`.
pub fn render_command(template: &str, file: &Path) -> String {
    // Single-quote the path and escape any embedded single quotes for the shell.
    let quoted = format!("'{}'", file.display().to_string().replace('\'', "'\\''"));
    template.replace("{file}", &quoted)
}

/// Run the transcription command on `file` and return the trimmed transcript.
pub fn run_transcription(template: &str, file: &Path) -> AppResult<String> {
    let rendered = render_command(template, file);
    let out = Command::new("sh")
        .arg("-c")
        .arg(&rendered)
        .output()
        .map_err(AppError::Io)?;
    if !out.status.success() {
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(AppError::Transcribe(if msg.is_empty() {
            "transcription command failed".to_string()
        } else {
            msg
        }));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn render_substitutes_quoted_file() {
        let rendered = render_command("whisper -f {file}", Path::new("/tmp/a b.wav"));
        assert_eq!(rendered, "whisper -f '/tmp/a b.wav'");
    }

    #[test]
    fn run_transcription_reads_command_stdout() {
        // A template that ignores the file and prints text.
        let dir = std::env::temp_dir();
        let text = run_transcription("printf 'hello world'", &dir).unwrap();
        assert_eq!(text, "hello world");
    }

    #[test]
    fn run_transcription_passes_the_file_through() {
        // `cat {file}` proves the audio path is substituted and reachable.
        let file = std::env::temp_dir().join(format!("autodev-tr-{}.txt", std::process::id()));
        fs::write(&file, "spoken words").unwrap();
        let text = run_transcription("cat {file}", &file).unwrap();
        assert_eq!(text, "spoken words");
        let _ = fs::remove_file(&file);
    }

    #[test]
    fn run_transcription_reports_failure() {
        assert!(run_transcription("exit 3", Path::new("/tmp")).is_err());
    }
}
