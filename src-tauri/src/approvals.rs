//! Per-action tool approval for interactive Rich sessions (B2), built on Claude Code's
//! `PreToolUse` hook — no MCP server, no SDK, no network port.
//!
//! For an interactive-approval session AutoDev generates a `--settings` file whose `PreToolUse`
//! hook is a small shell script (written per session). When the agent wants a tool, the hook
//! writes the request into a per-session approval dir and **blocks**, polling for a decision
//! file; the app surfaces Approve/Deny buttons and writes that file. On timeout the hook
//! fails safe with a deny. The transport is the filesystem, secured by user-only dir perms —
//! nothing listens on a socket.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use crate::error::{AppError, AppResult};

/// The `PreToolUse` hook script. `__DIR__`/`__TICKS__` are substituted per session. It writes
/// the tool request, waits for a `<req>.decision` file, and prints it; on timeout it denies.
const HOOK_SCRIPT: &str = r#"#!/usr/bin/env bash
# AutoDev per-action approval hook. Blocks the tool until the app writes a decision.
dir="__DIR__"
input="$(cat)"
req="$(mktemp "$dir/req.XXXXXX")"
printf '%s' "$input" > "$req"
for _ in $(seq 1 __TICKS__); do
  if [ -f "$req.decision" ]; then
    cat "$req.decision"; rm -f "$req" "$req.decision"; exit 0
  fi
  sleep 0.25
done
rm -f "$req"
printf '%s' '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"AutoDev: approval timed out"}}'
"#;

/// Paths produced by [`setup`]: the approval dir (polled for requests, written for decisions)
/// and the settings file to pass via `--settings`.
pub struct ApprovalSetup {
    pub dir: PathBuf,
    pub settings_path: PathBuf,
}

/// One tool call awaiting the user's decision.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingApproval {
    /// The request file name (`req.XXXXXX`); the id the UI responds with.
    pub id: String,
    pub tool_name: String,
    pub tool_input: Value,
}

/// Create the per-session approval dir, its blocking hook script, and the `--settings` file that
/// wires the hook in. `timeout_secs` is how long the hook waits before failing safe (deny).
pub fn setup(base_dir: &Path, agent_id: &str, timeout_secs: u32) -> AppResult<ApprovalSetup> {
    let dir = base_dir.join("approvals").join(agent_id);
    std::fs::create_dir_all(&dir)?;
    restrict_to_owner(&dir)?;

    let hook_path = dir.join("hook.sh");
    let ticks = (timeout_secs.max(1)) * 4; // one tick = 0.25s
    let script = HOOK_SCRIPT
        .replace("__DIR__", &dir.to_string_lossy())
        .replace("__TICKS__", &ticks.to_string());
    std::fs::write(&hook_path, script)?;
    make_executable(&hook_path)?;

    let settings = serde_json::json!({
        "hooks": {
            "PreToolUse": [{
                "matcher": "*",
                "hooks": [{ "type": "command", "command": hook_path.to_string_lossy() }]
            }]
        }
    });
    let settings_path = dir.join("settings.json");
    std::fs::write(&settings_path, serde_json::to_string_pretty(&settings)?)?;

    Ok(ApprovalSetup { dir, settings_path })
}

/// Requests written by the hook that don't yet have a decision. Reads `req.*` files, parsing the
/// tool name/input the hook captured. Best-effort: unreadable/partial files are skipped.
pub fn list_pending(dir: &Path) -> Vec<PendingApproval> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return vec![];
    };
    let mut out = Vec::new();
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if !name.starts_with("req.") || name.ends_with(".decision") {
            continue;
        }
        if dir.join(format!("{name}.decision")).exists() {
            continue; // already decided
        }
        let Ok(content) = std::fs::read_to_string(e.path()) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<Value>(&content) else {
            continue; // hook may still be writing it
        };
        out.push(PendingApproval {
            id: name,
            tool_name: v
                .get("tool_name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            tool_input: v.get("tool_input").cloned().unwrap_or(Value::Null),
        });
    }
    out
}

/// Write the decision file that unblocks the hook. `id` must be a bare request name we produced
/// (`req.…`) — validated to prevent a UI-supplied id escaping the approval dir (path traversal).
pub fn respond(dir: &Path, id: &str, allow: bool, reason: &str) -> AppResult<()> {
    if !id.starts_with("req.") || id.contains('/') || id.contains('\\') || id.contains("..") {
        return Err(AppError::NotFound(format!("invalid approval id {id}")));
    }
    let decision = serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": if allow { "allow" } else { "deny" },
            "permissionDecisionReason": reason,
        }
    });
    std::fs::write(
        dir.join(format!("{id}.decision")),
        serde_json::to_string(&decision)?,
    )?;
    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &Path) -> AppResult<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).map_err(AppError::Io)
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> AppResult<()> {
    Ok(())
}

#[cfg(unix)]
fn restrict_to_owner(dir: &Path) -> AppResult<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).map_err(AppError::Io)
}

#[cfg(not(unix))]
fn restrict_to_owner(_dir: &Path) -> AppResult<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "autodev-apr-{tag}-{:?}",
            std::thread::current().id()
        ));
        std::fs::remove_dir_all(&d).ok();
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// The hook input shape captured from a real `claude` PreToolUse invocation.
    fn write_request(dir: &Path, name: &str, tool: &str, cmd: &str) {
        let body = serde_json::json!({
            "session_id": "s1",
            "tool_name": tool,
            "tool_input": { "command": cmd },
        });
        std::fs::write(dir.join(name), serde_json::to_string(&body).unwrap()).unwrap();
    }

    #[test]
    fn setup_writes_an_executable_hook_and_a_settings_file_wiring_it() {
        let base = tmp("setup");
        let s = setup(&base, "agent-1", 120).unwrap();
        assert!(s.settings_path.exists());
        let hook = s.dir.join("hook.sh");
        assert!(hook.exists());
        let script = std::fs::read_to_string(&hook).unwrap();
        assert!(script.contains(&s.dir.to_string_lossy().to_string())); // __DIR__ substituted
        assert!(script.contains("480")); // 120s * 4 ticks
                                         // settings.json references the hook as a PreToolUse command with a wildcard matcher.
        let settings: Value =
            serde_json::from_str(&std::fs::read_to_string(&s.settings_path).unwrap()).unwrap();
        let entry = &settings["hooks"]["PreToolUse"][0];
        assert_eq!(entry["matcher"], "*");
        assert_eq!(
            entry["hooks"][0]["command"],
            hook.to_string_lossy().as_ref()
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&hook).unwrap().permissions().mode() & 0o111,
                0o100
            );
        }
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn list_pending_reads_requests_and_skips_decided_ones() {
        let base = tmp("list");
        let s = setup(&base, "a", 1).unwrap();
        write_request(&s.dir, "req.aaaaaa", "Bash", "echo hi");
        write_request(&s.dir, "req.bbbbbb", "Read", "x");

        let mut pending = list_pending(&s.dir);
        pending.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0].tool_name, "Bash");
        assert_eq!(
            pending[0].tool_input,
            serde_json::json!({ "command": "echo hi" })
        );

        // Decide one → it drops out of the pending list.
        respond(&s.dir, "req.aaaaaa", true, "ok").unwrap();
        let after: Vec<_> = list_pending(&s.dir).into_iter().map(|p| p.id).collect();
        assert_eq!(after, vec!["req.bbbbbb"]);
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn respond_writes_allow_or_deny_decision_json() {
        let base = tmp("respond");
        let s = setup(&base, "a", 1).unwrap();
        respond(&s.dir, "req.zzzzzz", true, "approved").unwrap();
        let d: Value = serde_json::from_str(
            &std::fs::read_to_string(s.dir.join("req.zzzzzz.decision")).unwrap(),
        )
        .unwrap();
        assert_eq!(d["hookSpecificOutput"]["permissionDecision"], "allow");
        assert_eq!(
            d["hookSpecificOutput"]["permissionDecisionReason"],
            "approved"
        );

        respond(&s.dir, "req.yyyyyy", false, "nope").unwrap();
        let d2: Value = serde_json::from_str(
            &std::fs::read_to_string(s.dir.join("req.yyyyyy.decision")).unwrap(),
        )
        .unwrap();
        assert_eq!(d2["hookSpecificOutput"]["permissionDecision"], "deny");
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn respond_rejects_path_traversal_ids() {
        let base = tmp("trav");
        let s = setup(&base, "a", 1).unwrap();
        for bad in ["../evil", "req.x/../../y", "/etc/passwd", "notareq"] {
            assert!(
                respond(&s.dir, bad, true, "x").is_err(),
                "should reject {bad}"
            );
        }
        std::fs::remove_dir_all(&base).ok();
    }
}
