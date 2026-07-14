use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::error::{AppError, AppResult};

/// Render a transcription command template by substituting `{file}` with the
/// (shell-quoted) audio path. The result is run through `sh -c`, so the template can be
/// any pipeline, e.g. `whisper-cli -f {file} -otxt -of {file} && cat {file}.txt`.
pub fn render_command(template: &str, file: &Path) -> String {
    // Single-quote the path and escape any embedded single quotes for the shell.
    let quoted = format!("'{}'", file.display().to_string().replace('\'', "'\\''"));
    template.replace("{file}", &quoted)
}

/// Run the transcription command, streaming the tool's stderr line-by-line to `on_progress`
/// as it runs, and returning the trimmed stdout transcript.
///
/// stderr is where a whisper-style tool prints what it is doing — first-run model downloads
/// (a `tqdm` bar updated with `\r`), the detected language, and each transcribed segment. We
/// split on both `\n` and `\r` so a carriage-return progress bar surfaces as discrete status
/// updates rather than one line that never breaks. stdout is read on a separate thread so a
/// tool that fills both pipes can't deadlock against us.
pub fn run_transcription_streaming(
    template: &str,
    file: &Path,
    mut on_progress: impl FnMut(&str),
) -> AppResult<String> {
    let rendered = render_command(template, file);
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(&rendered)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(AppError::Io)?;

    // Drain stdout (the transcript) on a thread so reading stderr for progress can't block on a
    // full stdout pipe, and vice-versa.
    let mut stdout = child.stdout.take().expect("piped stdout");
    let stdout_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        buf
    });

    // Read stderr on this thread, emitting each `\n`/`\r`-delimited token as a progress update.
    // Keep the last few lines so a failing command reports something useful.
    let mut stderr = child.stderr.take().expect("piped stderr");
    let mut tail: Vec<String> = Vec::new();
    let mut pending = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        match stderr.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                for &b in &chunk[..n] {
                    if b == b'\n' || b == b'\r' {
                        emit_token(&mut pending, &mut tail, &mut on_progress);
                    } else {
                        pending.push(b);
                    }
                }
            }
            Err(_) => break,
        }
    }
    emit_token(&mut pending, &mut tail, &mut on_progress);

    let status = child.wait().map_err(AppError::Io)?;
    let out = stdout_thread.join().unwrap_or_default();
    if !status.success() {
        let msg = tail.join("\n").trim().to_string();
        return Err(AppError::Transcribe(if msg.is_empty() {
            "transcription command failed".to_string()
        } else {
            msg
        }));
    }
    Ok(String::from_utf8_lossy(&out).trim().to_string())
}

/// Flush a completed stderr token: trim it, and if non-empty report it as progress and keep a
/// short tail for error reporting.
fn emit_token(pending: &mut Vec<u8>, tail: &mut Vec<String>, on_progress: &mut impl FnMut(&str)) {
    let line = String::from_utf8_lossy(pending).trim().to_string();
    pending.clear();
    if line.is_empty() {
        return;
    }
    on_progress(&line);
    tail.push(line);
    let overflow = tail.len().saturating_sub(8);
    if overflow > 0 {
        tail.drain(0..overflow);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Non-streaming convenience for tests that only care about the transcript.
    fn run(template: &str, file: &Path) -> AppResult<String> {
        run_transcription_streaming(template, file, |_| {})
    }

    #[test]
    fn render_substitutes_quoted_file() {
        let rendered = render_command("whisper -f {file}", Path::new("/tmp/a b.wav"));
        assert_eq!(rendered, "whisper -f '/tmp/a b.wav'");
    }

    #[test]
    fn run_transcription_reads_command_stdout() {
        // A template that ignores the file and prints text.
        let dir = std::env::temp_dir();
        let text = run("printf 'hello world'", &dir).unwrap();
        assert_eq!(text, "hello world");
    }

    #[test]
    fn run_transcription_passes_the_file_through() {
        // `cat {file}` proves the audio path is substituted and reachable.
        let file = std::env::temp_dir().join(format!("autodev-tr-{}.txt", std::process::id()));
        fs::write(&file, "spoken words").unwrap();
        let text = run("cat {file}", &file).unwrap();
        assert_eq!(text, "spoken words");
        let _ = fs::remove_file(&file);
    }

    #[test]
    fn run_transcription_reports_failure() {
        assert!(run("exit 3", Path::new("/tmp")).is_err());
    }

    #[test]
    fn streaming_returns_stdout_and_reports_stderr_progress() {
        // stderr carries progress (a plain line and a `\r`-updated one); stdout carries the
        // transcript. Both must come through, split on `\n` and `\r`.
        let mut seen = Vec::new();
        let text = run_transcription_streaming(
            "printf 'step one\\n' >&2; printf 'downloading 50%%\\r' >&2; printf 'the transcript'",
            Path::new("/tmp"),
            |l| seen.push(l.to_string()),
        )
        .unwrap();
        assert_eq!(text, "the transcript");
        assert!(seen.contains(&"step one".to_string()), "got {seen:?}");
        assert!(
            seen.contains(&"downloading 50%".to_string()),
            "got {seen:?}"
        );
    }

    #[test]
    fn streaming_surfaces_stderr_on_failure() {
        let err = run_transcription_streaming(
            "printf 'model not found\\n' >&2; exit 1",
            Path::new("/tmp"),
            |_| {},
        )
        .unwrap_err();
        assert!(format!("{err}").contains("model not found"));
    }
}
