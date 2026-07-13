use std::collections::HashMap;
use std::io::Read;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

/// Which coding-agent CLI to launch. The named variants are the backends the app ships
/// with; `Custom` is any backend registered by a disk spec (`~/.autodev/backends/*.json`),
/// so a new CLI needs no new variant. `Mock` runs an arbitrary command and exists so the
/// spawn/stream/write/exit path can be integration-tested without the real CLIs or their
/// auth (used in tests and CI).
///
/// Serialized transparently as its string `id` (e.g. `"claude"`), so the on-the-wire and
/// on-disk contract is unchanged from when this was a plain string enum.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentBackend {
    Claude,
    Codex,
    /// Google Antigravity's terminal agent, invoked as `agy`.
    Antigravity,
    Mock,
    /// A backend defined by a disk spec, identified by its `id`.
    Custom(String),
}

impl AgentBackend {
    /// The stable string id used to look up this backend's spec and to serialize it.
    pub fn id(&self) -> &str {
        match self {
            AgentBackend::Claude => "claude",
            AgentBackend::Codex => "codex",
            AgentBackend::Antigravity => "antigravity",
            AgentBackend::Mock => "mock",
            AgentBackend::Custom(id) => id,
        }
    }

    fn from_id(id: &str) -> Self {
        match id {
            "claude" => AgentBackend::Claude,
            "codex" => AgentBackend::Codex,
            "antigravity" => AgentBackend::Antigravity,
            "mock" => AgentBackend::Mock,
            other => AgentBackend::Custom(other.to_string()),
        }
    }
}

impl Serialize for AgentBackend {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.id())
    }
}

impl<'de> Deserialize<'de> for AgentBackend {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(AgentBackend::from_id(&String::deserialize(d)?))
    }
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
    /// Run the agent non-interactively (`claude -p`): it executes the prompt once, prints its
    /// result, and EXITS. The autonomous loop relies on this — its auto-advance fires on agent
    /// exit, and an interactive agent never exits.
    #[serde(default)]
    pub print_mode: bool,
    /// Launch in Rich mode: the backend emits a structured event stream (rendered as cards)
    /// instead of a raw terminal. Only honored for backends whose spec has a `structured`
    /// capability; ignored otherwise. Like print mode, this runs one-shot.
    #[serde(default)]
    pub rich: bool,
    #[serde(default)]
    pub model: Option<String>,
    /// Prompt to start the session with, passed as the CLI's positional prompt.
    #[serde(default)]
    pub initial_prompt: Option<String>,
    /// Extra directories to give the agent context on (from `@`-mentions). Passed as
    /// `--add-dir` to Claude Code.
    #[serde(default)]
    pub add_dirs: Vec<String>,
    /// Image files (annotated screenshots) to attach. Codex takes them as `-i`; Claude
    /// has no image CLI flag, so their paths are appended to the prompt instead.
    #[serde(default)]
    pub images: Vec<String>,
    /// For `Mock`: the command to run, e.g. `["bash", "script.sh"]`.
    #[serde(default)]
    pub mock_command: Option<Vec<String>>,
}

/// Compute the program and argument vector for a backend from its options. Pure and
/// therefore unit-tested; `build_command` wraps it into a `CommandBuilder`.
///
/// The real backends are described declaratively by a `BackendSpec` (see `backend_spec`),
/// so their flags live in data, not in this function. `Mock` is the one exception: it runs
/// an arbitrary command supplied by the test, which no spec describes.
///
/// Flags verified against the installed CLIs: `claude --permission-mode plan`,
/// `claude --dangerously-skip-permissions`, `claude --add-dir`;
/// `codex --dangerously-bypass-approvals-and-sandbox`. Antigravity (`agy`) flags follow
/// Google's published CLI guide (`-i`/`--prompt-interactive`, `-m`, `--add-dir`,
/// `--dangerously-skip-permissions`) — reconfirm against the installed `agy` version if they
/// drift; it is the only backend whose flags are not verified against a local install here.
pub fn command_line(
    opts: &AgentOptions,
    specs: &[crate::backend_spec::BackendSpec],
) -> AppResult<(String, Vec<String>)> {
    if let AgentBackend::Mock = opts.backend {
        let parts = opts
            .mock_command
            .as_ref()
            .filter(|v| !v.is_empty())
            .ok_or_else(|| AppError::NotFound("mock_command".to_string()))?;
        return Ok((parts[0].clone(), parts[1..].to_vec()));
    }
    let id = opts.backend.id();
    let spec = specs
        .iter()
        .find(|s| s.id == id)
        .ok_or_else(|| AppError::NotFound(format!("backend spec {id}")))?;
    Ok((spec.program.clone(), spec.build_args(opts)))
}

/// Build the process command line for a backend from its options, resolving its spec from
/// the bundled defaults plus any disk-registered backends (`~/.autodev/backends/*.json`).
pub fn build_command(opts: &AgentOptions) -> AppResult<CommandBuilder> {
    let (program, args) = command_line(opts, &crate::backend_spec::load_specs())?;
    let mut cmd = CommandBuilder::new(program);
    for a in args {
        cmd.arg(a);
    }
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
    use crate::backend_spec::builtin_specs;
    use std::sync::mpsc;
    use std::time::Duration;

    fn mock_opts(cmd: Vec<&str>) -> AgentOptions {
        AgentOptions {
            backend: AgentBackend::Mock,
            cwd: std::env::temp_dir().to_string_lossy().to_string(),
            plan_mode: false,
            bypass_permissions: false,
            print_mode: false,
            rich: false,
            model: None,
            initial_prompt: None,
            add_dirs: vec![],
            images: vec![],
            mock_command: Some(cmd.into_iter().map(String::from).collect()),
        }
    }

    /// A backend id round-trips through JSON as a bare string, and an unknown id becomes
    /// `Custom` — this is the wire contract the frontend relies on when it sends a
    /// disk-registered backend id.
    #[test]
    fn agent_backend_serde_roundtrips_including_custom() {
        for (variant, json) in [
            (AgentBackend::Claude, "\"claude\""),
            (AgentBackend::Mock, "\"mock\""),
            (AgentBackend::Custom("opencode".into()), "\"opencode\""),
        ] {
            assert_eq!(serde_json::to_string(&variant).unwrap(), json);
            assert_eq!(serde_json::from_str::<AgentBackend>(json).unwrap(), variant);
        }
    }

    /// The end-to-end P1 promise: dropping a JSON spec into `<data_dir>/backends/` makes a
    /// brand-new backend launchable — its `id` deserializes to `Custom`, `load_specs_from`
    /// registers it, and the real spawn arg-builder produces its command line. Zero Rust edits.
    #[test]
    fn a_disk_registered_backend_is_launchable_end_to_end() {
        let data =
            std::env::temp_dir().join(format!("autodev-dropin-{:?}", std::thread::current().id()));
        std::fs::create_dir_all(data.join("backends")).unwrap();
        std::fs::write(
            data.join("backends/opencode.json"),
            r#"{
                "id": "opencode",
                "label": "OpenCode",
                "program": "opencode",
                "bypassFlag": ["--yolo"],
                "modelFlag": "--model",
                "prompt": { "mode": "positional" }
            }"#,
        )
        .unwrap();

        // The backend id arrives from the frontend as a bare string.
        let backend: AgentBackend = serde_json::from_str("\"opencode\"").unwrap();
        assert_eq!(backend, AgentBackend::Custom("opencode".into()));

        let opts = AgentOptions {
            backend,
            cwd: "/tmp".into(),
            plan_mode: false,
            bypass_permissions: true,
            print_mode: false,
            rich: false,
            model: Some("big".into()),
            initial_prompt: Some("do it".into()),
            add_dirs: vec![],
            images: vec![],
            mock_command: None,
        };
        // Resolve through the same path spawn uses: specs loaded from disk.
        let specs = crate::backend_spec::load_specs_from(&data);
        let (program, args) = command_line(&opts, &specs).unwrap();
        assert_eq!(program, "opencode");
        assert_eq!(args, vec!["--yolo", "--model", "big", "do it"]);

        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn claude_print_mode_is_one_shot_and_skips_plan_mode() {
        let opts = AgentOptions {
            backend: AgentBackend::Claude,
            cwd: "/tmp".into(),
            plan_mode: true, // ignored in print mode
            bypass_permissions: false,
            print_mode: true,
            rich: false,
            model: None,
            initial_prompt: Some("do it".into()),
            add_dirs: vec![],
            images: vec![],
            mock_command: None,
        };
        let (program, args) = command_line(&opts, &builtin_specs()).unwrap();
        assert_eq!(program, "claude");
        assert_eq!(args, vec!["-p", "do it"]);
        assert!(!args.iter().any(|a| a == "--permission-mode"));
    }

    #[test]
    fn claude_command_line_maps_flags_and_add_dirs() {
        let opts = AgentOptions {
            backend: AgentBackend::Claude,
            cwd: "/tmp".into(),
            plan_mode: true,
            bypass_permissions: true,
            print_mode: false,
            rich: false,
            model: Some("claude-opus-4-8".into()),
            initial_prompt: Some("hello".into()),
            add_dirs: vec!["/a".into(), "/b".into()],
            images: vec![],
            mock_command: None,
        };
        let (program, args) = command_line(&opts, &builtin_specs()).unwrap();
        assert_eq!(program, "claude");
        assert_eq!(
            args,
            vec![
                "--permission-mode",
                "plan",
                "--dangerously-skip-permissions",
                "--model",
                "claude-opus-4-8",
                "--add-dir",
                "/a",
                "--add-dir",
                "/b",
                "hello",
            ]
        );
    }

    #[test]
    fn antigravity_maps_agy_flags_with_interactive_prompt() {
        let opts = AgentOptions {
            backend: AgentBackend::Antigravity,
            cwd: "/tmp".into(),
            plan_mode: true, // no agy plan flag → must be ignored, not emitted
            bypass_permissions: true,
            print_mode: false,
            rich: false,
            model: Some("gemini-3.1-pro".into()),
            initial_prompt: Some("build it".into()),
            add_dirs: vec!["/a".into()],
            images: vec!["/shots/a.png".into()],
            mock_command: None,
        };
        let (program, args) = command_line(&opts, &builtin_specs()).unwrap();
        assert_eq!(program, "agy");
        assert_eq!(
            args,
            vec![
                "--dangerously-skip-permissions",
                "-m",
                "gemini-3.1-pro",
                // The cwd is added to agy's workspace first, so it writes into the opened
                // project instead of a scratch sandbox, then any @-mentioned dirs follow.
                "--add-dir",
                "/tmp",
                "--add-dir",
                "/a",
                "-i",
                "build it\n\n[Screenshot attached: /shots/a.png]",
            ]
        );
        // No plan flag leaked in.
        assert!(!args.iter().any(|a| a.contains("plan")));
    }

    #[test]
    fn antigravity_adds_cwd_to_workspace() {
        // Even a bare session must put the working directory in agy's workspace, or agy
        // writes deliverables into its own scratch project instead of the opened folder.
        let opts = AgentOptions {
            backend: AgentBackend::Antigravity,
            cwd: "/work/zlog".into(),
            plan_mode: false,
            bypass_permissions: false,
            print_mode: false,
            rich: false,
            model: None,
            initial_prompt: None,
            add_dirs: vec![],
            images: vec![],
            mock_command: None,
        };
        let (program, args) = command_line(&opts, &builtin_specs()).unwrap();
        assert_eq!(program, "agy");
        assert_eq!(args, vec!["--add-dir", "/work/zlog"]);
    }

    #[test]
    fn antigravity_does_not_duplicate_cwd_in_add_dirs() {
        // If the cwd also appears in add_dirs, it must be emitted once, not twice.
        let opts = AgentOptions {
            backend: AgentBackend::Antigravity,
            cwd: "/work/zlog".into(),
            plan_mode: false,
            bypass_permissions: false,
            print_mode: false,
            rich: false,
            model: None,
            initial_prompt: None,
            add_dirs: vec!["/work/zlog".into(), "/other".into()],
            images: vec![],
            mock_command: None,
        };
        let (_, args) = command_line(&opts, &builtin_specs()).unwrap();
        assert_eq!(args, vec!["--add-dir", "/work/zlog", "--add-dir", "/other"]);
    }

    #[test]
    fn codex_maps_images_and_claude_appends_them_to_prompt() {
        let codex = AgentOptions {
            backend: AgentBackend::Codex,
            cwd: "/tmp".into(),
            plan_mode: false,
            bypass_permissions: false,
            print_mode: false,
            rich: false,
            model: None,
            initial_prompt: Some("look".into()),
            add_dirs: vec![],
            images: vec!["/shots/a.png".into()],
            mock_command: None,
        };
        let (_, args) = command_line(&codex, &builtin_specs()).unwrap();
        assert_eq!(args, vec!["-i", "/shots/a.png", "look"]);

        let claude = AgentOptions {
            backend: AgentBackend::Claude,
            images: vec!["/shots/a.png".into()],
            ..codex
        };
        let (_, args) = command_line(&claude, &builtin_specs()).unwrap();
        assert_eq!(args.len(), 1);
        assert!(args[0].contains("look"));
        assert!(args[0].contains("[Screenshot attached: /shots/a.png]"));
    }

    #[test]
    fn codex_command_line_maps_bypass_and_model() {
        let opts = AgentOptions {
            backend: AgentBackend::Codex,
            cwd: "/tmp".into(),
            plan_mode: false,
            bypass_permissions: true,
            print_mode: false,
            rich: false,
            model: Some("o3".into()),
            initial_prompt: None,
            add_dirs: vec![],
            images: vec![],
            mock_command: None,
        };
        let (program, args) = command_line(&opts, &builtin_specs()).unwrap();
        assert_eq!(program, "codex");
        assert_eq!(
            args,
            vec!["--dangerously-bypass-approvals-and-sandbox", "-m", "o3"]
        );
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
