//! Microphone capture in the Rust core.
//!
//! WebKitGTK's `MediaRecorder` (the Linux webview) is unreliable — it can return an empty or
//! undecodable file — so recording lives here instead of the frontend, matching the repo rule
//! that process/hardware access belongs to the core. A configurable command (default: ffmpeg
//! capturing PulseAudio to a 16 kHz mono WAV) writes the mic to a file; stopping writes `q` to
//! its stdin so ffmpeg quits and finalizes the container, escalating to a kill if it lingers.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use crate::error::{AppError, AppResult};

/// Default capture command when the user has set none: ffmpeg from the PulseAudio default source
/// to a 16 kHz mono WAV (what whisper wants), quiet unless it errors.
pub const DEFAULT_RECORD_COMMAND: &str =
    "ffmpeg -hide_banner -loglevel error -f pulse -i default -ar 16000 -ac 1 -y {file}";

/// A capture in progress: the child process writing audio and the file it writes to.
pub struct Recording {
    child: Child,
    pub file: PathBuf,
}

/// App-managed slot holding the single active recording, if any.
#[derive(Default)]
pub struct RecorderState(pub Mutex<Option<Recording>>);

/// Spawn the capture command, writing mic audio to `file`. stdin is piped so `finalize` can ask
/// the tool to quit gracefully.
pub fn spawn(template: &str, file: &Path) -> AppResult<Recording> {
    let rendered = crate::transcribe::render_command(template, file);
    let child = Command::new("sh")
        .arg("-c")
        .arg(&rendered)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(AppError::Io)?;
    Ok(Recording {
        child,
        file: file.to_path_buf(),
    })
}

/// Stop a recording and wait for the file to be finalized. Writes `q` to the tool's stdin
/// (ffmpeg's graceful quit, which flushes the container trailer), waits briefly, then kills the
/// process if it has not exited. Returns the path that now holds the finished recording.
pub fn finalize(mut recording: Recording) -> PathBuf {
    if let Some(mut stdin) = recording.child.stdin.take() {
        let _ = stdin.write_all(b"q\n");
        // Dropping stdin closes the pipe, which also nudges tools that quit on EOF.
    }
    for _ in 0..40 {
        match recording.child.try_wait() {
            Ok(Some(_)) => return recording.file,
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(_) => break,
        }
    }
    let _ = recording.child.kill();
    let _ = recording.child.wait();
    recording.file
}

/// Kill any active recording without transcribing — used on app shutdown so no ffmpeg is orphaned.
pub fn kill(state: &RecorderState) {
    if let Ok(mut guard) = state.0.lock() {
        if let Some(mut recording) = guard.take() {
            let _ = recording.child.kill();
            let _ = recording.child.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finalize_stops_a_process_that_ignores_stdin() {
        // `sleep` ignores the `q` and stdin close; finalize must escalate to a kill and return.
        let file = std::env::temp_dir().join("autodev-rec-test.wav");
        let rec = spawn("sleep 30", &file).unwrap();
        let returned = finalize(rec);
        assert_eq!(returned, file);
    }

    #[test]
    fn finalize_lets_a_cooperative_process_exit_on_its_own() {
        // `cat` to a file exits when stdin closes — the graceful path, no kill needed.
        let file = std::env::temp_dir().join(format!("autodev-rec-{}.out", std::process::id()));
        let template = format!("cat > '{}'", file.display());
        let rec = spawn(&template, &file).unwrap();
        let returned = finalize(rec);
        assert_eq!(returned, file);
        let _ = std::fs::remove_file(&file);
    }
}
