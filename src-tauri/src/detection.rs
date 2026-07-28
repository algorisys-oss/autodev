//! Agent state detection: what is this agent *doing*, read from its terminal output.
//!
//! A terminal agent tells you its state only by what it draws. This module turns the raw PTY
//! byte stream into one of three semantic states — working, waiting on the user, or idle — so
//! the UI can show it and unattended loops can act on it. It lives in the core (not the UI)
//! because it is a read of process output, and it is pure: feed bytes, ask for a state at a
//! given instant. No threads, no clock, no Tauri.
//!
//! Two things make this more than a regex over the last few bytes:
//!
//! 1. **A screen snapshot, not a byte tail.** TUI agents redraw: they clear the screen and
//!    repaint. A tail of the byte stream therefore holds text that is no longer on screen — a
//!    permission prompt the agent already answered still sits in the tail and reads as
//!    "waiting" forever. [`Screen`] applies the erase/clear sequences and carriage-return
//!    overwrites, so what it holds is what the user can actually see.
//! 2. **Positive evidence for `working`.** Silence does not mean idle: an agent thinking for
//!    thirty seconds emits nothing, but its footer still says `esc to interrupt`. Matching that
//!    footer keeps it `Working` through the silence instead of flipping it to `Idle`.
//!
//! Patterns are per-backend data ([`DetectionSpec`] on `BackendSpec`), not code, so a backend
//! added from `<data_dir>/backends/*.json` can carry its own signatures. A backend with no spec
//! is never classified `Waiting` from another backend's patterns — an unrecognized screen falls
//! back to `Idle` rather than guessing.

use serde::{Deserialize, Serialize};

/// Milliseconds since the first call, from a monotonic clock. The core owns time (a wall clock
/// that steps backwards would otherwise make a working agent look quiet).
pub fn now_ms() -> u64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_millis() as u64
}

/// How long output must have stopped before silence is read as a state rather than a pause
/// between writes. Matches the cadence agents redraw their spinners at.
pub const QUIET_AFTER_MS: u64 = 1_500;

/// How often states are recomputed.
pub const TICK_MS: u64 = 500;

/// What an agent is doing, as read from its screen. Terminal outcomes (exited / crashed) are
/// not here: those come from the process exit code, which is authoritative.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentState {
    /// Mid-turn: producing output, or showing its own busy footer.
    Working,
    /// Parked on the user — an approval menu, a y/n prompt, a question.
    Waiting,
    /// Quiet, with nothing on screen asking for the user.
    Idle,
}

/// Per-backend detection signatures. Case-insensitive substrings, not regexes: every verified
/// signature is a literal phrase (`esc to interrupt`, `Do you want to proceed?`), so a substring
/// match covers them without a regex dependency. Structural shapes that cannot be written as a
/// literal get their own flag ([`Self::menu_cursor`]).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectionSpec {
    /// Positive evidence the agent is mid-turn — its own busy footer. Checked first, so a
    /// long silent think stays `Working`.
    #[serde(default)]
    pub busy: Vec<String>,
    /// Evidence the agent is parked on the user: approval, permission, or question UI.
    #[serde(default)]
    pub blocked: Vec<String>,
    /// Also treat a selection cursor on a numbered option (`❯ 1. Yes`) as blocked — the shape
    /// Claude Code and Codex render approval menus in. Structural, so it is a flag rather than
    /// a literal.
    #[serde(default)]
    pub menu_cursor: bool,
}

impl DetectionSpec {
    fn matches_any(patterns: &[String], haystack_lower: &str) -> bool {
        patterns
            .iter()
            .any(|p| !p.is_empty() && haystack_lower.contains(&p.to_lowercase()))
    }
}

/// True if a line is a selection cursor on a numbered option — `❯ 1. Yes`, `> 2) No`. The
/// cursor glyph, optional space, digits, then `.` or `)` and a space.
fn has_menu_cursor(line: &str) -> bool {
    let mut chars = line.chars().peekable();
    let Some(first) = chars.next() else {
        return false;
    };
    if !matches!(first, '❯' | '▶' | '►' | '›' | '➤' | '>') {
        return false;
    }
    while chars.peek() == Some(&' ') {
        chars.next();
    }
    let mut digits = 0;
    while chars.peek().is_some_and(|c| c.is_ascii_digit()) {
        chars.next();
        digits += 1;
    }
    if digits == 0 {
        return false;
    }
    if !matches!(chars.next(), Some('.') | Some(')')) {
        return false;
    }
    chars.next() == Some(' ')
}

/// Split `buf` into the longest decodable UTF-8 prefix and the undecodable tail. A tail that is
/// merely an incomplete trailing sequence is returned for the caller to hold; genuinely invalid
/// bytes are consumed as replacement characters so a corrupt stream cannot wedge the parser.
fn decode_utf8_prefix(buf: &[u8]) -> (String, &[u8]) {
    let mut out = String::new();
    let mut rest = buf;
    loop {
        match std::str::from_utf8(rest) {
            Ok(s) => {
                out.push_str(s);
                return (out, &[]);
            }
            Err(e) => {
                let valid = e.valid_up_to();
                out.push_str(std::str::from_utf8(&rest[..valid]).unwrap_or(""));
                match e.error_len() {
                    // Incomplete trailing sequence — hold it for the next chunk.
                    None => return (out, &rest[valid..]),
                    // Genuinely invalid bytes: show a replacement char and keep decoding, so
                    // valid text after the corruption is not stuck behind it.
                    Some(bad) => {
                        out.push('\u{fffd}');
                        rest = &rest[valid + bad..];
                    }
                }
            }
        }
    }
}

/// How many trailing screen lines the classifier looks at. A prompt the agent is parked on is
/// at the bottom; older text higher up is history, not current state.
const SCAN_LINES: usize = 8;

/// Cap on retained screen text, so a chatty agent cannot grow this without bound.
const MAX_SCREEN_BYTES: usize = 8 * 1024;

/// The visible bottom of an agent's terminal, reconstructed from its byte stream.
///
/// Not a full VT emulator — it handles the sequences that actually change what is on screen for
/// a coding agent's TUI: erase-display (`ESC[2J`, `ESC[J`), erase-line (`ESC[2K`, `ESC[K`),
/// carriage-return overwrites, and the OSC title (`ESC]0;…`), which agents set to announce what
/// they are doing. Everything else (colour, cursor movement) is dropped, as it does not change
/// which *text* is present.
#[derive(Debug, Default)]
pub struct Screen {
    /// Completed lines, oldest first.
    lines: Vec<String>,
    /// The line currently being written.
    current: String,
    /// Bytes buffered mid-escape-sequence, so a sequence split across two PTY chunks parses.
    pending: Vec<u8>,
    /// The last terminal title the agent set, if any.
    title: Option<String>,
}

impl Screen {
    pub fn new() -> Self {
        Self::default()
    }

    /// The last title the agent set via OSC 0/1/2, if it ever set one.
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// The visible text: completed lines plus the line in progress, newest last.
    pub fn text(&self) -> String {
        let mut out = self.lines.join("\n");
        if !self.current.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&self.current);
        }
        out
    }

    /// The last [`SCAN_LINES`] non-blank visible lines — what the classifier reads.
    pub fn tail(&self) -> String {
        let text = self.text();
        let lines: Vec<&str> = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        let start = lines.len().saturating_sub(SCAN_LINES);
        lines[start..].join("\n")
    }

    fn newline(&mut self) {
        let line = std::mem::take(&mut self.current);
        self.lines.push(line);
        // Keep only enough history to serve `tail()`, plus slack for redraws.
        while self.lines.len() > SCAN_LINES * 8 {
            self.lines.remove(0);
        }
        while self.byte_len() > MAX_SCREEN_BYTES && !self.lines.is_empty() {
            self.lines.remove(0);
        }
    }

    fn byte_len(&self) -> usize {
        self.lines.iter().map(|l| l.len() + 1).sum::<usize>() + self.current.len()
    }

    fn clear(&mut self) {
        self.lines.clear();
        self.current.clear();
    }

    /// Feed raw PTY bytes.
    pub fn feed(&mut self, bytes: &[u8]) {
        let mut buf = std::mem::take(&mut self.pending);
        buf.extend_from_slice(bytes);
        // Decode only the complete UTF-8 prefix; a multi-byte glyph split across two PTY chunks
        // must be held whole rather than lossily replaced.
        let (text, leftover) = decode_utf8_prefix(&buf);
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            match c {
                '\x1b' => match self.consume_escape(&chars, i) {
                    Some(next) => i = next,
                    None => {
                        // Sequence is incomplete — keep it, and the undecoded tail, for the
                        // next chunk.
                        let mut rest = chars[i..].iter().collect::<String>().into_bytes();
                        rest.extend_from_slice(leftover);
                        self.pending = rest;
                        return;
                    }
                },
                '\n' => {
                    self.newline();
                    i += 1;
                }
                // A PTY converts every newline to CRLF, so a `\r` immediately before a `\n` is
                // a line ending, not a redraw. Clearing on it would blank every line the agent
                // prints. A lone `\r` (the spinner rewriting its row) still clears.
                '\r' if chars.get(i + 1) == Some(&'\n') => {
                    self.newline();
                    i += 2;
                }
                // A `\r` at the very end of a chunk may be the first half of a CRLF whose `\n`
                // is in the next one — hold it rather than guessing.
                '\r' if i + 1 == chars.len() => {
                    let mut rest = vec![b'\r'];
                    rest.extend_from_slice(leftover);
                    self.pending = rest;
                    return;
                }
                '\r' => {
                    self.current.clear();
                    i += 1;
                }
                '\x08' => {
                    self.current.pop();
                    i += 1;
                }
                c if (c as u32) < 0x20 => i += 1, // other control chars draw nothing
                c => {
                    self.current.push(c);
                    i += 1;
                }
            }
        }
        self.pending = leftover.to_vec();
    }

    /// Handle the escape sequence starting at `chars[i]` (which is ESC). Returns the index just
    /// past it, or `None` when the sequence is not yet complete.
    fn consume_escape(&mut self, chars: &[char], i: usize) -> Option<usize> {
        match chars.get(i + 1) {
            None => None,
            Some('[') => {
                // CSI: parameters, then a final byte in 0x40..=0x7e.
                let mut j = i + 2;
                while j < chars.len() && !matches!(chars[j], '\x40'..='\x7e') {
                    j += 1;
                }
                let final_byte = *chars.get(j)?;
                let params: String = chars[i + 2..j].iter().collect();
                self.apply_csi(&params, final_byte);
                Some(j + 1)
            }
            Some(']') => {
                // OSC: ESC ] <ps> ; <text> terminated by BEL or ST (ESC \).
                let mut j = i + 2;
                let mut end = None;
                while j < chars.len() {
                    if chars[j] == '\x07' {
                        end = Some((j, j + 1));
                        break;
                    }
                    if chars[j] == '\x1b' && chars.get(j + 1) == Some(&'\\') {
                        end = Some((j, j + 2));
                        break;
                    }
                    j += 1;
                }
                let (text_end, next) = end?;
                let body: String = chars[i + 2..text_end].iter().collect();
                self.apply_osc(&body);
                Some(next)
            }
            Some(_) => Some(i + 2), // other two-character escape
        }
    }

    fn apply_csi(&mut self, params: &str, final_byte: char) {
        let n = params.trim_start_matches('?').parse::<u32>().unwrap_or(0);
        match final_byte {
            // Erase in display: 2 (whole screen) and 3 (scrollback) reset what is visible.
            'J' if n >= 2 => self.clear(),
            // Erase from cursor to end of display: everything below is gone; the line stays.
            'J' => self.lines.clear(),
            // Erase in line: 2 (whole line) or 0/absent (cursor to end) — the agent is
            // repainting this row, so drop what it held.
            'K' => self.current.clear(),
            _ => {}
        }
    }

    fn apply_osc(&mut self, body: &str) {
        // `0;title` (icon + title), `1;title` (icon), `2;title` (title).
        let Some((ps, text)) = body.split_once(';') else {
            return;
        };
        if matches!(ps, "0" | "1" | "2") {
            self.title = Some(text.to_string());
        }
    }
}

/// Tracks one agent's state: a [`Screen`] plus the time of its last output.
///
/// Pure — the caller supplies the clock. `feed` records activity, `state` answers "what is it
/// doing at this instant", given how long silence must last before it counts as quiet.
#[derive(Debug)]
pub struct StateTracker {
    screen: Screen,
    spec: DetectionSpec,
    last_output_ms: u64,
}

impl StateTracker {
    pub fn new(spec: DetectionSpec, now_ms: u64) -> Self {
        Self {
            screen: Screen::new(),
            spec,
            last_output_ms: now_ms,
        }
    }

    pub fn feed(&mut self, bytes: &[u8], now_ms: u64) {
        self.screen.feed(bytes);
        self.last_output_ms = now_ms;
    }

    /// The agent's state now. `quiet_after_ms` is how long output must have stopped before
    /// silence is treated as meaningful.
    ///
    /// Order matters, and it is the whole design:
    /// 1. A busy footer on screen ⇒ `Working`, even mid-silence (a silent think is still work).
    /// 2. Blocked UI on screen ⇒ `Waiting`, but only while the agent is actually quiet — a
    ///    prompt scrolling past mid-stream is history, not a parked agent.
    /// 3. Recent output ⇒ `Working`.
    /// 4. Otherwise `Idle`. Never `Waiting` on an unrecognized screen: a missed prompt shows a
    ///    wrong dot, a false one sends the user chasing an agent that needs nothing.
    pub fn state(&self, now_ms: u64, quiet_after_ms: u64) -> AgentState {
        let tail = self.screen.tail();
        let lower = tail.to_lowercase();
        if DetectionSpec::matches_any(&self.spec.busy, &lower) {
            return AgentState::Working;
        }
        let quiet = now_ms.saturating_sub(self.last_output_ms) >= quiet_after_ms;
        if quiet && self.is_blocked(&tail, &lower) {
            return AgentState::Waiting;
        }
        if !quiet {
            return AgentState::Working;
        }
        AgentState::Idle
    }

    fn is_blocked(&self, tail: &str, lower: &str) -> bool {
        if DetectionSpec::matches_any(&self.spec.blocked, lower) {
            return true;
        }
        self.spec.menu_cursor && tail.lines().any(has_menu_cursor)
    }

    /// The last terminal title the agent set — a free state signal agents publish and that the
    /// UI shows as the card's activity line.
    pub fn title(&self) -> Option<&str> {
        self.screen.title()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> DetectionSpec {
        DetectionSpec {
            busy: vec!["esc to interrupt".into()],
            blocked: vec!["do you want to proceed?".into()],
            menu_cursor: true,
        }
    }

    // --- Screen: the snapshot must be what is on screen, not what was streamed ---

    #[test]
    fn clearing_the_screen_drops_text_that_is_no_longer_visible() {
        let mut s = Screen::new();
        s.feed(b"Do you want to proceed?\n");
        assert!(s.text().contains("Do you want to proceed?"));
        s.feed(b"\x1b[2JRunning tests\n");
        assert!(!s.text().contains("Do you want to proceed?"));
        assert!(s.text().contains("Running tests"));
    }

    #[test]
    fn carriage_return_and_erase_line_overwrite_the_row_being_redrawn() {
        let mut s = Screen::new();
        s.feed(b"thinking 1s\rthinking 2s");
        assert_eq!(s.text(), "thinking 2s");
        s.feed(b"\x1b[2Kdone");
        assert_eq!(s.text(), "done");
    }

    #[test]
    fn erase_to_end_of_display_drops_the_lines_below() {
        let mut s = Screen::new();
        s.feed(b"old menu\nstale prompt\n");
        s.feed(b"\x1b[Jfresh\n");
        assert!(!s.text().contains("stale prompt"));
        assert!(s.text().contains("fresh"));
    }

    #[test]
    fn escape_sequences_split_across_chunks_still_parse() {
        let mut s = Screen::new();
        s.feed(b"before\n\x1b");
        s.feed(b"[2Jafter");
        assert_eq!(s.text(), "after");
    }

    #[test]
    fn multibyte_utf8_split_across_chunks_is_not_corrupted() {
        let mut s = Screen::new();
        let glyph = "❯".as_bytes();
        s.feed(&glyph[..1]);
        s.feed(&glyph[1..]);
        s.feed(b" 1. Yes");
        assert_eq!(s.text(), "❯ 1. Yes");
    }

    #[test]
    fn osc_title_is_captured_from_either_terminator() {
        let mut s = Screen::new();
        s.feed(b"\x1b]0;claude: editing agent.rs\x07work");
        assert_eq!(s.title(), Some("claude: editing agent.rs"));
        assert_eq!(s.text(), "work");
        s.feed(b"\x1b]2;codex: running tests\x1b\\");
        assert_eq!(s.title(), Some("codex: running tests"));
    }

    #[test]
    fn colour_codes_do_not_appear_in_the_snapshot() {
        let mut s = Screen::new();
        s.feed(b"\x1b[1m\x1b[36mSelect an option\x1b[0m");
        assert_eq!(s.text(), "Select an option");
    }

    #[test]
    fn tail_keeps_only_the_last_visible_lines() {
        let mut s = Screen::new();
        for i in 0..40 {
            s.feed(format!("line {i}\n").as_bytes());
        }
        let tail = s.tail();
        assert!(tail.contains("line 39"));
        assert!(!tail.contains("line 20"));
    }

    #[test]
    fn screen_does_not_grow_without_bound() {
        let mut s = Screen::new();
        for i in 0..5000 {
            s.feed(format!("a fairly long line of agent output number {i}\n").as_bytes());
        }
        assert!(s.text().len() <= MAX_SCREEN_BYTES + 1024);
    }

    // --- Classification ---

    #[test]
    fn a_busy_footer_holds_working_through_a_long_silent_think() {
        let mut t = StateTracker::new(spec(), 0);
        t.feed(b"Thinking... (esc to interrupt)", 0);
        // 30s of total silence — a byte-tail-and-timer classifier would call this idle.
        assert_eq!(t.state(30_000, 1_500), AgentState::Working);
    }

    #[test]
    fn recent_output_is_working_even_without_a_busy_footer() {
        let mut t = StateTracker::new(spec(), 0);
        t.feed(b"compiling...", 1_000);
        assert_eq!(t.state(1_200, 1_500), AgentState::Working);
    }

    #[test]
    fn a_quiet_agent_parked_on_a_prompt_is_waiting() {
        let mut t = StateTracker::new(spec(), 0);
        t.feed(b"Do you want to proceed?\n", 0);
        assert_eq!(t.state(5_000, 1_500), AgentState::Waiting);
    }

    #[test]
    fn a_quiet_agent_parked_on_a_selection_menu_is_waiting() {
        let mut t = StateTracker::new(spec(), 0);
        t.feed("Approve?\n\u{276f} 1. Yes\n  2. No\n".as_bytes(), 0);
        assert_eq!(t.state(5_000, 1_500), AgentState::Waiting);
    }

    #[test]
    fn a_prompt_the_agent_already_answered_does_not_stick_as_waiting() {
        let mut t = StateTracker::new(spec(), 0);
        t.feed(b"Do you want to proceed?\n", 0);
        // The agent answers and repaints the screen; the prompt is gone from view.
        t.feed(b"\x1b[2JEditing src/main.rs\n", 100);
        assert_eq!(t.state(5_000, 1_500), AgentState::Idle);
    }

    /// A PTY rewrites every `\n` as `\r\n`. Treating that `\r` as a redraw blanks every line the
    /// agent prints, leaving an empty screen that classifies as idle no matter what is on it.
    /// These are the exact bytes captured from a PTY-hosted agent.
    #[test]
    fn crlf_line_endings_from_a_pty_do_not_blank_the_screen() {
        let mut s = Screen::new();
        s.feed(b"Analyzing the codebase\r\n\x1b[2K\r* Thinking... (esc to interrupt)");
        s.feed(b"\x1b[2JEdit file src/main.rs\r\nDo you want to proceed?\r\n");
        s.feed("\u{276f} 1. Yes\r\n  2. No, and tell Claude what to do\r\n".as_bytes());
        let text = s.text();
        assert!(text.contains("Do you want to proceed?"), "got: {text:?}");
        assert!(text.contains("❯ 1. Yes"), "got: {text:?}");

        let mut t = StateTracker::new(spec(), 0);
        t.feed(b"Do you want to proceed?\r\n\xe2\x9d\xaf 1. Yes\r\n", 0);
        assert_eq!(t.state(5_000, 1_500), AgentState::Waiting);
    }

    /// A CRLF split across two PTY reads is still one line ending, not a redraw plus a newline.
    #[test]
    fn a_crlf_split_across_chunks_is_still_one_line_ending() {
        let mut s = Screen::new();
        s.feed(b"Do you want to proceed?\r");
        s.feed(b"\n\xe2\x9d\xaf 1. Yes");
        assert_eq!(s.text(), "Do you want to proceed?\n❯ 1. Yes");
    }

    /// A lone `\r` is still a redraw: it is how a spinner rewrites its own row in place.
    #[test]
    fn a_lone_carriage_return_still_redraws_the_row() {
        let mut s = Screen::new();
        s.feed(b"thinking 1s\rthinking 2s\r\n");
        assert_eq!(s.text(), "thinking 2s");
    }

    #[test]
    fn a_quiet_agent_with_nothing_asking_is_idle() {
        let mut t = StateTracker::new(spec(), 0);
        t.feed(b"All 42 tests passed.\n", 0);
        assert_eq!(t.state(5_000, 1_500), AgentState::Idle);
    }

    #[test]
    fn ordinary_output_that_merely_mentions_a_prompt_is_not_waiting() {
        let mut t = StateTracker::new(spec(), 0);
        t.feed(b"Step 1. clone the repo\nWhat is the meaning of life?\n", 0);
        assert_eq!(t.state(5_000, 1_500), AgentState::Idle);
    }

    #[test]
    fn a_backend_with_no_spec_is_never_classified_waiting() {
        let mut t = StateTracker::new(DetectionSpec::default(), 0);
        t.feed("Do you want to proceed?\n\u{276f} 1. Yes\n".as_bytes(), 0);
        // Another backend's signatures must not leak into an unknown one.
        assert_eq!(t.state(5_000, 1_500), AgentState::Idle);
    }

    #[test]
    fn a_prompt_mid_stream_is_history_not_a_parked_agent() {
        let mut t = StateTracker::new(spec(), 0);
        t.feed(b"Do you want to proceed?\n", 0);
        // Still streaming: not quiet yet, so it is working rather than waiting.
        assert_eq!(t.state(200, 1_500), AgentState::Working);
    }

    #[test]
    fn menu_cursor_shape_is_matched_structurally() {
        assert!(has_menu_cursor("❯ 1. Yes"));
        assert!(has_menu_cursor("> 2) No, and tell Claude"));
        assert!(has_menu_cursor("➤3. Maybe"));
        assert!(!has_menu_cursor("Step 1. clone the repo"));
        assert!(!has_menu_cursor("> git status"));
        assert!(!has_menu_cursor("❯ yes please"));
    }

    #[test]
    fn detection_spec_round_trips_through_json() {
        let json = r#"{"busy":["esc to interrupt"],"blocked":["proceed?"],"menuCursor":true}"#;
        let parsed: DetectionSpec = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.busy, vec!["esc to interrupt".to_string()]);
        assert!(parsed.menu_cursor);
        let back = serde_json::to_string(&parsed).unwrap();
        assert!(back.contains("menuCursor"));
    }

    #[test]
    fn an_absent_detection_spec_deserializes_to_no_signatures() {
        let parsed: DetectionSpec = serde_json::from_str("{}").unwrap();
        assert_eq!(parsed, DetectionSpec::default());
    }
}
