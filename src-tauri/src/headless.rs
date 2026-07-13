//! Headless / RPC mode (P6): drive the orchestrator over JSONL on stdin/stdout, no GUI.
//!
//! One long-running process. Each stdin line is a `Command` (`{"cmd":"spawn",…}`); each stdout
//! line is an `Event` (`{"event":"output",…}`). A shell script can pipe commands in and read
//! events out — the same spawn→prompt→observe→kill cycle the GUI drives, scriptable. This reuses
//! the Tauri-independent core (`agent::spawn_session`): the on_output/on_exit callbacks that feed
//! the GUI's Tauri events here feed JSONL instead. When stdin closes, every child is killed
//! (ProcessManager owns lifetimes — LOOPS XXXVIII).

use std::io::{self, BufRead, Write};
use std::sync::Arc;

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::agent::{AgentInfo, AgentManager, AgentOptions};

/// A command read from stdin.
#[derive(Debug, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase")]
enum Command {
    /// Launch an agent. `options` is the same `AgentOptions` the GUI sends.
    Spawn { options: Box<AgentOptions> },
    /// Send input (keystrokes / a prompt) to an agent's stdin.
    Write { id: String, data: String },
    /// Kill an agent.
    Kill { id: String },
    /// List live agents.
    List,
}

/// An event written to stdout.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "camelCase")]
enum Event {
    Spawned {
        id: String,
    },
    /// base64 of raw PTY bytes (escape sequences preserved), matching the GUI's `agent://output`.
    Output {
        id: String,
        data: String,
    },
    Exit {
        id: String,
        code: Option<i32>,
    },
    List {
        agents: Vec<AgentInfo>,
    },
    Error {
        message: String,
    },
}

/// Emits one JSONL event. `Send + Sync` so the per-agent reader thread can emit through it.
type Emit = Arc<dyn Fn(&Event) + Send + Sync>;

const B64: base64::engine::general_purpose::GeneralPurpose =
    base64::engine::general_purpose::STANDARD;

/// Run the headless loop against real stdin/stdout until EOF, then kill every agent.
pub fn run() {
    let emit: Emit = Arc::new(|e: &Event| {
        if let Ok(s) = serde_json::to_string(e) {
            let mut out = io::stdout().lock();
            let _ = writeln!(out, "{s}");
            let _ = out.flush();
        }
    });
    let manager = Arc::new(AgentManager::default());
    let stdin = io::stdin();
    for line in stdin.lock().lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        dispatch(&manager, &emit, &line);
    }
    manager.kill_all();
}

/// Parse and act on one command line, emitting events. Separated from `run` so tests can drive
/// it with a collecting emitter and the `Mock` backend, no real CLIs or stdio.
fn dispatch(manager: &Arc<AgentManager>, emit: &Emit, line: &str) {
    let cmd: Command = match serde_json::from_str(line) {
        Ok(c) => c,
        Err(e) => {
            return emit(&Event::Error {
                message: format!("bad command: {e}"),
            })
        }
    };
    match cmd {
        Command::Spawn { options } => spawn(manager, emit, *options),
        Command::Write { id, data } => match manager.get(&id) {
            Ok(s) => {
                if let Err(e) = s.write(data.as_bytes()) {
                    emit(&Event::Error {
                        message: e.to_string(),
                    });
                }
            }
            Err(e) => emit(&Event::Error {
                message: e.to_string(),
            }),
        },
        Command::Kill { id } => match manager.get(&id) {
            Ok(s) => {
                let _ = s.kill();
            }
            Err(e) => emit(&Event::Error {
                message: e.to_string(),
            }),
        },
        Command::List => emit(&Event::List {
            agents: manager.list(),
        }),
    }
}

fn spawn(manager: &Arc<AgentManager>, emit: &Emit, options: AgentOptions) {
    let id = manager.next_id();

    let out_emit = emit.clone();
    let out_id = id.clone();
    let on_output = move |bytes: Vec<u8>| {
        out_emit(&Event::Output {
            id: out_id.clone(),
            data: B64.encode(&bytes),
        });
    };

    let exit_emit = emit.clone();
    let exit_id = id.clone();
    let on_exit = move |code: Option<i32>| {
        exit_emit(&Event::Exit {
            id: exit_id.clone(),
            code,
        });
    };

    match crate::agent::spawn_session(id.clone(), &options, 80, 24, on_output, on_exit) {
        Ok(session) => {
            manager.insert(session);
            emit(&Event::Spawned { id });
        }
        Err(e) => emit(&Event::Error {
            message: e.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    fn collector() -> (Emit, mpsc::Receiver<Event>) {
        let (tx, rx) = mpsc::channel();
        let emit: Emit = Arc::new(move |e: &Event| {
            let _ = tx.send(e.clone());
        });
        (emit, rx)
    }

    fn mock_spawn(cmd: &str) -> String {
        format!(
            r#"{{"cmd":"spawn","options":{{"backend":"mock","cwd":"/tmp","mockCommand":["bash","-c","{cmd}"]}}}}"#
        )
    }

    #[test]
    fn a_bad_command_line_emits_an_error_not_a_panic() {
        let manager = Arc::new(AgentManager::default());
        let (emit, rx) = collector();
        dispatch(&manager, &emit, "this is not json");
        match rx.recv_timeout(Duration::from_secs(1)).unwrap() {
            Event::Error { message } => assert!(message.contains("bad command")),
            e => panic!("expected Error, got {e:?}"),
        }
    }

    #[test]
    fn spawn_streams_output_then_exit_the_full_cycle() {
        let manager = Arc::new(AgentManager::default());
        let (emit, rx) = collector();
        dispatch(
            &manager,
            &emit,
            &mock_spawn("printf hello-headless; exit 0"),
        );

        let mut spawned = false;
        let mut out = String::new();
        let mut exit_code = None;
        while let Ok(e) = rx.recv_timeout(Duration::from_secs(5)) {
            match e {
                Event::Spawned { .. } => spawned = true,
                Event::Output { data, .. } => {
                    out.push_str(&String::from_utf8_lossy(&B64.decode(data).unwrap()))
                }
                Event::Exit { code, .. } => {
                    exit_code = Some(code);
                    break;
                }
                other => panic!("unexpected event {other:?}"),
            }
        }
        assert!(spawned, "should announce the spawn");
        assert!(
            out.contains("hello-headless"),
            "should stream output, got {out:?}"
        );
        assert_eq!(exit_code, Some(Some(0)), "should report a clean exit");
    }

    #[test]
    fn write_reaches_the_agent_and_list_reflects_it() {
        let manager = Arc::new(AgentManager::default());
        let (emit, rx) = collector();
        dispatch(&manager, &emit, &mock_spawn("cat")); // echoes stdin back
        let id = loop {
            match rx.recv_timeout(Duration::from_secs(5)).unwrap() {
                Event::Spawned { id } => break id,
                _ => continue,
            }
        };

        dispatch(&manager, &emit, r#"{"cmd":"list"}"#);
        let listed = loop {
            match rx.recv_timeout(Duration::from_secs(2)).unwrap() {
                Event::List { agents } => break agents,
                _ => continue,
            }
        };
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, id);

        dispatch(
            &manager,
            &emit,
            &format!(r#"{{"cmd":"write","id":"{id}","data":"ping\n"}}"#),
        );
        let mut echoed = String::new();
        while let Ok(e) = rx.recv_timeout(Duration::from_secs(5)) {
            if let Event::Output { data, .. } = e {
                echoed.push_str(&String::from_utf8_lossy(&B64.decode(data).unwrap()));
                if echoed.contains("ping") {
                    break;
                }
            }
        }
        assert!(
            echoed.contains("ping"),
            "write should reach the agent, got {echoed:?}"
        );
        dispatch(&manager, &emit, &format!(r#"{{"cmd":"kill","id":"{id}"}}"#));
    }

    #[test]
    fn write_to_a_missing_agent_emits_an_error() {
        let manager = Arc::new(AgentManager::default());
        let (emit, rx) = collector();
        dispatch(
            &manager,
            &emit,
            r#"{"cmd":"write","id":"ghost","data":"x"}"#,
        );
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(1)).unwrap(),
            Event::Error { .. }
        ));
    }
}
