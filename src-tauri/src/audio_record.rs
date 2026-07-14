//! Microphone capture in the Rust core.
//!
//! WebKitGTK's `MediaRecorder` (the Linux webview) is unreliable — it can return an empty or
//! undecodable file — so recording lives here instead of the frontend, matching the repo rule
//! that process/hardware access belongs to the core. A configurable command (default: a per-OS
//! ffmpeg capture to a 16 kHz mono WAV — PulseAudio/ALSA on Linux, avfoundation on macOS) writes
//! the mic to a file; stopping writes `q` to its stdin so ffmpeg quits and finalizes the
//! container, escalating to a kill if it lingers.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use crate::error::{AppError, AppResult};

/// A capture command to use when the user hasn't set one, picked for the current OS (and, on
/// Linux, the available audio server). All variants use ffmpeg to write a 16 kHz mono WAV — what
/// whisper wants — quiet unless it errors. `{file}` is the output path. `None` if ffmpeg is not
/// installed (or the OS has no wired-up default), so the caller can prompt the user.
pub fn default_record_command() -> Option<String> {
    if !crate::capture::program_on_path("ffmpeg") {
        return None;
    }
    let input: &str = record_input_args();
    if input.is_empty() {
        return None;
    }
    Some(format!(
        "ffmpeg -hide_banner -loglevel error {input} -ar 16000 -ac 1 -y {{file}}"
    ))
}

/// The ffmpeg input flags for capturing the default microphone on this platform. On Linux this
/// prefers a PulseAudio/PipeWire server (detected by its socket) and falls back to ALSA. Empty
/// when there is no sensible default (e.g. Windows dshow needs a device name the user must set).
fn record_input_args() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        // avfoundation index `:0` is the default audio input (no video).
        "-f avfoundation -i :0"
    }
    #[cfg(target_os = "linux")]
    {
        if pulse_available() {
            "-f pulse -i default"
        } else {
            "-f alsa -i default"
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        ""
    }
}

/// Whether a PulseAudio-compatible server (PulseAudio or PipeWire's pulse shim) is reachable,
/// detected without needing the `pactl` CLI: an explicit `PULSE_SERVER`, or the native socket
/// under `XDG_RUNTIME_DIR`.
#[cfg(target_os = "linux")]
fn pulse_available() -> bool {
    if std::env::var_os("PULSE_SERVER").is_some() {
        return true;
    }
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(|rt| Path::new(&rt).join("pulse/native").exists())
        .unwrap_or(false)
}

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
    fn default_record_command_is_well_formed_when_present() {
        // ffmpeg may or may not be installed on CI; if a default is offered it must be a valid,
        // substitutable ffmpeg template.
        if let Some(t) = default_record_command() {
            assert!(
                t.contains("{file}"),
                "template must have a {{file}} slot: {t}"
            );
            assert!(t.contains("ffmpeg"), "default capture uses ffmpeg: {t}");
            assert!(t.contains("-ar 16000"), "captures 16 kHz for whisper: {t}");
        }
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
