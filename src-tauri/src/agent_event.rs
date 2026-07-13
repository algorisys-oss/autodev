//! Normalized, backend-agnostic agent events + the driver that produces them.
//!
//! A structured backend (one whose CLI can emit machine-readable events) is rendered in the
//! Rich view as a stream of [`AgentEvent`]s rather than raw terminal bytes. Each backend has a
//! [`StructuredDriver`] that translates its native protocol into this one model, so the UI
//! renders a single shape regardless of which CLI produced it — this enum is the multi-backend
//! contract and is mirrored in the frontend `ipc.ts`.
//!
//! Increment 1 ships the Claude driver ([`ClaudeStreamJsonDriver`]) for `claude -p
//! --output-format stream-json --verbose`. The stream is newline-delimited JSON; shapes here
//! were captured from `claude` 2.1.207.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One thing an agent did during a session, normalized across backends. Serialized with a
/// `kind` discriminator (camelCase) — the exact wire contract the frontend depends on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AgentEvent {
    /// Session metadata, emitted once at start.
    SessionInit {
        model: String,
        cwd: String,
        permission_mode: String,
    },
    /// A block of assistant-visible prose.
    AssistantText { text: String },
    /// The agent's internal reasoning (extended thinking), when the backend exposes it.
    Thinking { text: String },
    /// The agent invoked a tool. `input` is the raw tool arguments as JSON; the UI summarizes it.
    ToolCall {
        id: String,
        name: String,
        input: Value,
    },
    /// The result of a previously-called tool.
    ToolResult {
        tool_use_id: String,
        ok: bool,
        output: String,
    },
    /// The session finished; `text` is its final result string.
    Done {
        ok: bool,
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cost_usd: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
    },
    /// A line the driver could not parse — surfaced rather than dropped, so nothing is lost.
    Raw { text: String },
}

/// Translates a backend's raw stdout bytes into a stream of [`AgentEvent`]s. Fed incrementally
/// on the PTY reader thread, so it must buffer partial trailing lines across calls.
pub trait StructuredDriver: Send {
    fn feed(&mut self, bytes: &[u8]) -> Vec<AgentEvent>;
}

/// The driver for `claude ... --output-format stream-json` (newline-delimited JSON).
#[derive(Default)]
pub struct ClaudeStreamJsonDriver {
    /// Bytes received but not yet terminated by a newline. Buffered as bytes (not a decoded
    /// string) so a multi-byte UTF-8 sequence split across two PTY chunks isn't corrupted.
    buf: Vec<u8>,
}

impl StructuredDriver for ClaudeStreamJsonDriver {
    fn feed(&mut self, bytes: &[u8]) -> Vec<AgentEvent> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        while let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.buf.drain(..=pos).collect();
            let text = String::from_utf8_lossy(&line);
            let text = text.trim();
            if !text.is_empty() {
                out.extend(parse_line(text));
            }
        }
        out
    }
}

/// Construct the driver named by a backend spec's `structured.driver`, if known.
pub fn driver_for(name: &str) -> Option<Box<dyn StructuredDriver>> {
    match name {
        "claudeStreamJson" => Some(Box::<ClaudeStreamJsonDriver>::default()),
        _ => None,
    }
}

/// Parse one complete NDJSON line into zero or more normalized events. Unrecognized envelope
/// types (e.g. `rate_limit_event`) produce nothing; a line that isn't valid JSON becomes `Raw`.
fn parse_line(line: &str) -> Vec<AgentEvent> {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return vec![AgentEvent::Raw {
            text: line.to_string(),
        }];
    };
    match v.get("type").and_then(Value::as_str) {
        Some("system") if v.get("subtype").and_then(Value::as_str) == Some("init") => {
            vec![AgentEvent::SessionInit {
                model: str_field(&v, "model"),
                cwd: str_field(&v, "cwd"),
                permission_mode: str_field(&v, "permissionMode"),
            }]
        }
        // Assistant turns carry text/thinking/tool_use blocks; the synthetic user turn in
        // print mode carries tool_result blocks. Both live under message.content.
        Some("assistant") | Some("user") => parse_content(v.get("message")),
        Some("result") => vec![AgentEvent::Done {
            ok: !v.get("is_error").and_then(Value::as_bool).unwrap_or(false),
            text: str_field(&v, "result"),
            cost_usd: v.get("total_cost_usd").and_then(Value::as_f64),
            duration_ms: v.get("duration_ms").and_then(Value::as_u64),
        }],
        _ => vec![],
    }
}

fn parse_content(message: Option<&Value>) -> Vec<AgentEvent> {
    let Some(blocks) = message
        .and_then(|m| m.get("content"))
        .and_then(Value::as_array)
    else {
        return vec![];
    };
    let mut out = Vec::new();
    for b in blocks {
        match b.get("type").and_then(Value::as_str) {
            Some("text") => {
                let text = block_str(b, "text");
                if !text.is_empty() {
                    out.push(AgentEvent::AssistantText { text });
                }
            }
            Some("thinking") => {
                let text = block_str(b, "thinking");
                if !text.is_empty() {
                    out.push(AgentEvent::Thinking { text });
                }
            }
            Some("tool_use") => out.push(AgentEvent::ToolCall {
                id: block_str(b, "id"),
                name: block_str(b, "name"),
                input: b.get("input").cloned().unwrap_or(Value::Null),
            }),
            Some("tool_result") => out.push(AgentEvent::ToolResult {
                tool_use_id: block_str(b, "tool_use_id"),
                ok: !b.get("is_error").and_then(Value::as_bool).unwrap_or(false),
                output: flatten_tool_output(b.get("content")),
            }),
            _ => {}
        }
    }
    out
}

/// A tool result's `content` is either a plain string or an array of `{type,text}` blocks
/// (structured results). Flatten both to displayable text.
fn flatten_tool_output(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|b| b.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn str_field(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn block_str(b: &Value, key: &str) -> String {
    b.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real lines captured from `claude 2.1.207 -p "…" --output-format stream-json --verbose`,
    // trimmed to the fields the driver reads. If Claude's schema drifts, these fail.
    const INIT: &str = r#"{"type":"system","subtype":"init","cwd":"/proj","session_id":"s1","model":"claude-opus-4-8[1m]","permissionMode":"bypassPermissions","tools":["Read","Bash"]}"#;
    const RATE_LIMIT: &str =
        r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed"},"session_id":"s1"}"#;
    const ASSISTANT_TEXT: &str = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"hello from stream json."}]},"session_id":"s1"}"#;
    const ASSISTANT_THINKING: &str = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":"let me think","signature":"x"}]},"session_id":"s1"}"#;
    const ASSISTANT_TOOL_USE: &str = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_01","name":"Read","input":{"file_path":"/proj/sample.txt"},"caller":{"type":"direct"}}]},"session_id":"s1"}"#;
    const USER_TOOL_RESULT: &str = r#"{"type":"user","message":{"role":"user","content":[{"tool_use_id":"toolu_01","type":"tool_result","content":"1\thello-file-contents\n2\t"}]},"session_id":"s1"}"#;
    const RESULT: &str = r#"{"type":"result","subtype":"success","is_error":false,"duration_ms":5372,"num_turns":1,"result":"hello from stream json.","total_cost_usd":0.0682375,"session_id":"s1"}"#;

    fn feed_lines(lines: &[&str]) -> Vec<AgentEvent> {
        let mut d = ClaudeStreamJsonDriver::default();
        d.feed(format!("{}\n", lines.join("\n")).as_bytes())
    }

    #[test]
    fn text_turn_yields_init_assistant_and_done() {
        let evs = feed_lines(&[INIT, RATE_LIMIT, ASSISTANT_TEXT, RESULT]);
        assert_eq!(
            evs,
            vec![
                AgentEvent::SessionInit {
                    model: "claude-opus-4-8[1m]".into(),
                    cwd: "/proj".into(),
                    permission_mode: "bypassPermissions".into(),
                },
                AgentEvent::AssistantText {
                    text: "hello from stream json.".into()
                },
                AgentEvent::Done {
                    ok: true,
                    text: "hello from stream json.".into(),
                    cost_usd: Some(0.0682375),
                    duration_ms: Some(5372),
                },
            ]
        );
    }

    #[test]
    fn tool_turn_yields_toolcall_then_toolresult() {
        let evs = feed_lines(&[ASSISTANT_TOOL_USE, USER_TOOL_RESULT]);
        assert_eq!(
            evs,
            vec![
                AgentEvent::ToolCall {
                    id: "toolu_01".into(),
                    name: "Read".into(),
                    input: serde_json::json!({ "file_path": "/proj/sample.txt" }),
                },
                AgentEvent::ToolResult {
                    tool_use_id: "toolu_01".into(),
                    ok: true,
                    output: "1\thello-file-contents\n2\t".into(),
                },
            ]
        );
    }

    #[test]
    fn thinking_block_maps_to_thinking_event() {
        assert_eq!(
            feed_lines(&[ASSISTANT_THINKING]),
            vec![AgentEvent::Thinking {
                text: "let me think".into()
            }]
        );
    }

    #[test]
    fn tool_result_content_array_is_flattened_to_text() {
        let line = r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t2","content":[{"type":"text","text":"line a"},{"type":"text","text":"line b"}]}]}}"#;
        assert_eq!(
            feed_lines(&[line]),
            vec![AgentEvent::ToolResult {
                tool_use_id: "t2".into(),
                ok: true,
                output: "line a\nline b".into(),
            }]
        );
    }

    #[test]
    fn tool_result_error_flag_sets_not_ok() {
        let line = r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t3","is_error":true,"content":"boom"}]}}"#;
        assert_eq!(
            feed_lines(&[line]),
            vec![AgentEvent::ToolResult {
                tool_use_id: "t3".into(),
                ok: false,
                output: "boom".into(),
            }]
        );
    }

    #[test]
    fn rate_limit_and_unknown_envelope_types_produce_no_events() {
        assert!(feed_lines(&[RATE_LIMIT]).is_empty());
        assert!(feed_lines(&[r#"{"type":"something_new","x":1}"#]).is_empty());
    }

    #[test]
    fn partial_line_is_buffered_until_its_newline_arrives() {
        let mut d = ClaudeStreamJsonDriver::default();
        let full = format!("{ASSISTANT_TEXT}\n");
        let (head, tail) = full.split_at(40); // split mid-JSON, before the newline
        assert!(d.feed(head.as_bytes()).is_empty(), "no complete line yet");
        let evs = d.feed(tail.as_bytes());
        assert_eq!(
            evs,
            vec![AgentEvent::AssistantText {
                text: "hello from stream json.".into()
            }]
        );
    }

    #[test]
    fn multibyte_utf8_split_across_chunks_is_not_corrupted() {
        // "café ☕" — the é (2 bytes) and ☕ (3 bytes) can straddle a chunk boundary.
        let line =
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"café ☕"}]}}"#;
        let bytes = format!("{line}\n").into_bytes();
        let mut d = ClaudeStreamJsonDriver::default();
        let mut evs = Vec::new();
        for chunk in bytes.chunks(7) {
            evs.extend(d.feed(chunk));
        }
        assert_eq!(
            evs,
            vec![AgentEvent::AssistantText {
                text: "café ☕".into()
            }]
        );
    }

    #[test]
    fn unparseable_line_becomes_raw() {
        assert_eq!(
            feed_lines(&["this is not json"]),
            vec![AgentEvent::Raw {
                text: "this is not json".into()
            }]
        );
    }

    #[test]
    fn multiple_events_across_separate_feeds_accumulate_in_order() {
        let mut d = ClaudeStreamJsonDriver::default();
        let mut evs = d.feed(format!("{INIT}\n").as_bytes());
        evs.extend(d.feed(format!("{ASSISTANT_TEXT}\n").as_bytes()));
        assert_eq!(evs.len(), 2);
        assert!(matches!(evs[0], AgentEvent::SessionInit { .. }));
        assert!(matches!(evs[1], AgentEvent::AssistantText { .. }));
    }

    /// Locks the wire contract the frontend mirrors: the `kind` tag and camelCase fields.
    #[test]
    fn events_serialize_with_kind_tag_and_camelcase() {
        let json = serde_json::to_value(AgentEvent::AssistantText { text: "hi".into() }).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "kind": "assistantText", "text": "hi" })
        );

        let done = serde_json::to_value(AgentEvent::Done {
            ok: true,
            text: "done".into(),
            cost_usd: None,
            duration_ms: Some(10),
        })
        .unwrap();
        // Absent optionals are omitted, not null.
        assert_eq!(
            done,
            serde_json::json!({ "kind": "done", "ok": true, "text": "done", "durationMs": 10 })
        );
    }
}
