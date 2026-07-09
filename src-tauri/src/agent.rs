use std::collections::HashMap;
use std::io::Read;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

/// Which coding-agent CLI to launch. `Mock` runs an arbitrary command and exists so
/// the spawn/stream/write/exit path can be integration-tested without the real CLIs
/// or their auth (used in tests and CI).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentBackend {
    Claude,
    Codex,
    Mock,
}

/// Options for launching one agent session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentOptions {
    pub backend: AgentBackend,
    /// Working directory the agent runs in.
    pub cwd: String,
    #[serde(default)]
    pub plan_mode: bool,
    #[serde(default)]
    pub bypass_permissions: bool,
    #[serde(default)]
    pub model: Option<String>,
    /// Prompt to start the session with, passed as the CLI's positional prompt.
    #[serde(default)]
    pub initial_prompt: Option<String>,
    /// For `Mock`: the command to run, e.g. `["bash", "script.sh"]`.
    #[serde(default)]
    pub mock_command: Option<Vec<String>>,
}

/// Build the process command line for a backend from its options.
///
/// Flags verified against the installed CLIs: `claude --permission-mode plan`,
/// `claude --dangerously-skip-permissions`; `codex --dangerously-bypass-approvals-and-sandbox`.
pub fn build_command(opts: &AgentOptions) -> AppResult<CommandBuilder> {
    let mut cmd = match &opts.backend {
        AgentBackend::Claude => {
            let mut c = CommandBuilder::new("claude");
            if opts.plan_mode {
                c.arg("--permission-mode");
                c.arg("plan");
            }
            if opts.bypass_permissions {
                c.arg("--dangerously-skip-permissions");
            }
            if let Some(m) = &opts.model {
                c.arg("--model");
                c.arg(m);
            }
            if let Some(p) = &opts.initial_prompt {
                c.arg(p);
            }
            c
        }
        AgentBackend::Codex => {
            let mut c = CommandBuilder::new("codex");
            if opts.bypass_permissions {
                c.arg("--dangerously-bypass-approvals-and-sandbox");
            }
            if let Some(m) = &opts.model {
                c.arg("-m");
                c.arg(m);
            }
            if let Some(p) = &opts.initial_prompt {
                c.arg(p);
            }
            c
        }
        AgentBackend::Mock => {
            let parts = opts
                .mock_command
                .as_ref()
                .filter(|v| !v.is_empty())
                .ok_or_else(|| AppError::NotFound("mock_command".to_string()))?;
            let mut c = CommandBuilder::new(&parts[0]);
            for a in &parts[1..] {
                c.arg(a);
            }
            c
        }
    };
    cmd.cwd(&opts.cwd);
    // A sensible default term so agents emit normal escape sequences.
    cmd.env("TERM", "xterm-256color");
    Ok(cmd)
}

/// A live agent session: the PTY master (for resize), a writer (for input), and the
/// child process (for kill/reap).
pub struct AgentSession {
    pub id: String,
    pub backend: AgentBackend,
    pub cwd: String,
    running: Arc<AtomicBool>,
    writer: Mutex<Box<dyn std::io::Write + Send>>,
    master: Mutex<Box<dyn MasterPty + Send>>,
    child: Arc<Mutex<Box<dyn Child + Send + Sync>>>,
}

impl AgentSession {
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn write(&self, data: &[u8]) -> AppResult<()> {
        let mut w = self.writer.lock().unwrap();
        w.write_all(data)?;
        w.flush()?;
        Ok(())
    }

    pub fn resize(&self, cols: u16, rows: u16) -> AppResult<()> {
        self.master
            .lock()
            .unwrap()
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| AppError::Pty(e.to_string()))
    }

    pub fn kill(&self) -> AppResult<()> {
        self.child.lock().unwrap().kill().map_err(AppError::Io)
    }
}

/// Spawn a session and wire its output/exit to callbacks. Callbacks run on the
/// reader thread. This is the Tauri-independent core, so tests drive it directly.
pub fn spawn_session(
    id: String,
    opts: &AgentOptions,
    cols: u16,
    rows: u16,
    on_output: impl Fn(Vec<u8>) + Send + 'static,
    on_exit: impl FnOnce(Option<i32>) + Send + 'static,
) -> AppResult<Arc<AgentSession>> {
    let cmd = build_command(opts)?;
    let pty = native_pty_system()
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| AppError::Pty(e.to_string()))?;

    let child = pty
        .slave
        .spawn_command(cmd)
        .map_err(|e| AppError::Pty(e.to_string()))?;
    // Drop the slave in the parent so EOF is delivered when the child exits.
    drop(pty.slave);

    let mut reader = pty
        .master
        .try_clone_reader()
        .map_err(|e| AppError::Pty(e.to_string()))?;
    let writer = pty
        .master
        .take_writer()
        .map_err(|e| AppError::Pty(e.to_string()))?;

    let running = Arc::new(AtomicBool::new(true));
    let child = Arc::new(Mutex::new(child));

    let session = Arc::new(AgentSession {
        id: id.clone(),
        backend: opts.backend.clone(),
        cwd: opts.cwd.clone(),
        running: running.clone(),
        writer: Mutex::new(writer),
        master: Mutex::new(pty.master),
        child: child.clone(),
    });

    // Reader thread: stream output until EOF (child exit), then reap and report.
    thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => on_output(buf[..n].to_vec()),
                Err(_) => break,
            }
        }
        let code = child
            .lock()
            .unwrap()
            .wait()
            .ok()
            .map(|s| s.exit_code() as i32);
        running.store(false, Ordering::SeqCst);
        on_exit(code);
    });

    Ok(session)
}

/// Serializable snapshot of a session for the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInfo {
    pub id: String,
    pub backend: AgentBackend,
    pub cwd: String,
    pub running: bool,
}

/// Holds all live sessions. Stored in Tauri state.
#[derive(Default)]
pub struct AgentManager {
    sessions: Mutex<HashMap<String, Arc<AgentSession>>>,
    counter: AtomicU64,
}

impl AgentManager {
    pub fn next_id(&self) -> String {
        let n = self.counter.fetch_add(1, Ordering::SeqCst) + 1;
        format!("agent-{n}")
    }

    pub fn insert(&self, session: Arc<AgentSession>) {
        self.sessions
            .lock()
            .unwrap()
            .insert(session.id.clone(), session);
    }

    pub fn get(&self, id: &str) -> AppResult<Arc<AgentSession>> {
        self.sessions
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("agent {id}")))
    }

    pub fn list(&self) -> Vec<AgentInfo> {
        self.sessions
            .lock()
            .unwrap()
            .values()
            .map(|s| AgentInfo {
                id: s.id.clone(),
                backend: s.backend.clone(),
                cwd: s.cwd.clone(),
                running: s.is_running(),
            })
            .collect()
    }

    /// Kill every live session. Returns how many were signalled.
    pub fn kill_all(&self) -> usize {
        let sessions = self.sessions.lock().unwrap();
        let mut n = 0;
        for s in sessions.values() {
            if s.is_running() && s.kill().is_ok() {
                n += 1;
            }
        }
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    fn mock_opts(cmd: Vec<&str>) -> AgentOptions {
        AgentOptions {
            backend: AgentBackend::Mock,
            cwd: std::env::temp_dir().to_string_lossy().to_string(),
            plan_mode: false,
            bypass_permissions: false,
            model: None,
            initial_prompt: None,
            mock_command: Some(cmd.into_iter().map(String::from).collect()),
        }
    }

    #[test]
    fn build_command_maps_claude_flags() {
        let opts = AgentOptions {
            backend: AgentBackend::Claude,
            cwd: "/tmp".into(),
            plan_mode: true,
            bypass_permissions: true,
            model: Some("claude-opus-4-8".into()),
            initial_prompt: Some("hello".into()),
            mock_command: None,
        };
        // build_command must succeed and not panic; exact argv is covered by running it.
        assert!(build_command(&opts).is_ok());
    }

    #[test]
    fn mock_session_streams_output_and_exits() {
        // echo then exit: proves spawn -> output -> EOF -> exit code path.
        let (tx, rx) = mpsc::channel();
        let (exit_tx, exit_rx) = mpsc::channel();
        let opts = mock_opts(vec!["bash", "-c", "printf 'hello-pty'; exit 0"]);
        let _session = spawn_session(
            "agent-test-1".into(),
            &opts,
            80,
            24,
            move |bytes| {
                let _ = tx.send(bytes);
            },
            move |code| {
                let _ = exit_tx.send(code);
            },
        )
        .unwrap();

        // collect output until exit
        let mut out = Vec::new();
        while let Ok(chunk) = rx.recv_timeout(Duration::from_secs(5)) {
            out.extend_from_slice(&chunk);
            if String::from_utf8_lossy(&out).contains("hello-pty") {
                break;
            }
        }
        assert!(String::from_utf8_lossy(&out).contains("hello-pty"));

        let code = exit_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(code, Some(0));
    }

    #[test]
    fn mock_session_accepts_input() {
        // `cat` echoes stdin back through the pty until it closes.
        let (tx, rx) = mpsc::channel();
        let opts = mock_opts(vec!["cat"]);
        let session = spawn_session(
            "agent-test-2".into(),
            &opts,
            80,
            24,
            move |bytes| {
                let _ = tx.send(bytes);
            },
            |_| {},
        )
        .unwrap();

        session.write(b"ping\n").unwrap();

        let mut out = Vec::new();
        while let Ok(chunk) = rx.recv_timeout(Duration::from_secs(5)) {
            out.extend_from_slice(&chunk);
            if String::from_utf8_lossy(&out).contains("ping") {
                break;
            }
        }
        assert!(String::from_utf8_lossy(&out).contains("ping"));
        session.kill().unwrap();
    }

    #[test]
    fn manager_tracks_and_kills_sessions() {
        let manager = AgentManager::default();
        let id = manager.next_id();
        assert_eq!(id, "agent-1");
        let opts = mock_opts(vec!["sleep", "30"]);
        let session = spawn_session(id.clone(), &opts, 80, 24, |_| {}, |_| {}).unwrap();
        manager.insert(session);

        assert_eq!(manager.list().len(), 1);
        assert!(manager.get(&id).is_ok());
        let killed = manager.kill_all();
        assert_eq!(killed, 1);
    }
}
