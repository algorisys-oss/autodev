use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

/// The roles never share a context (LOOPS XXVIII). Each has its own system prompt.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Breaks the whole spec into an ordered feature backlog (the epic driver).
    Decomposer,
    Planner,
    Generator,
    Evaluator,
}

/// Where a loop is in its life cycle.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoopPhase {
    /// Decomposer is breaking the spec into a feature backlog (epic start).
    Decomposing,
    /// Planner is turning the current feature into a contract.
    Planning,
    /// Generator is implementing against the contract.
    Generating,
    /// Evaluator is grading the diff against the contract.
    Evaluating,
    /// Every feature in the backlog is done.
    Passed,
    /// A feature stalled or ran out of iterations.
    Failed,
}

/// One increment of the backlog the epic works through, in order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Feature {
    pub title: String,
    pub done: bool,
}

/// One testable "done" assertion (LOOPS XXIX). `met` is `None` until graded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Criterion {
    pub text: String,
    pub met: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopState {
    pub id: String,
    pub spec: String,
    pub project_dir: String,
    pub phase: LoopPhase,
    pub iteration: u32,
    pub max_iterations: u32,
    pub contract: Vec<Criterion>,
    /// The ordered feature backlog the epic works through (empty until the decomposer runs).
    pub features: Vec<Feature>,
    /// Index of the feature currently being planned/built.
    #[serde(default)]
    pub current_feature: usize,
    pub progress: String,
    /// HEAD commit captured when the current generation round began; the evaluator diffs the
    /// work tree against it. `None` until the loop first enters Generating (older saved state
    /// loads with `None` via serde default).
    #[serde(default)]
    pub base_commit: Option<String>,
    /// Ground-truth test command (run via `sh -c` in the project dir). Exit 0 = the build's own
    /// tests pass; a failure blocks `Passed` even when the evaluator says every criterion is met.
    #[serde(default)]
    pub verify_command: Option<String>,
    /// Met-criterion count after each graded round — the signal for stuck detection.
    #[serde(default)]
    pub history: Vec<u32>,
    /// Why the loop reached `Failed` (out of iterations / stuck / tests failing). `None` otherwise.
    #[serde(default)]
    pub failure_reason: Option<String>,
}

/// Number of consecutive rounds with no new progress that marks a loop as stuck.
pub const STUCK_WINDOW: usize = 3;
/// Default iteration budget (raised from the original 5 now that stuck-detection ends dead loops).
pub const DEFAULT_MAX_ITERATIONS: u32 = 8;

impl LoopState {
    pub fn with_options(
        id: String,
        spec: String,
        project_dir: String,
        verify_command: Option<String>,
        max_iterations: u32,
    ) -> Self {
        Self {
            id,
            spec,
            project_dir,
            phase: LoopPhase::Decomposing,
            iteration: 0,
            max_iterations: max_iterations.max(1),
            contract: Vec::new(),
            features: Vec::new(),
            current_feature: 0,
            progress: String::new(),
            base_commit: None,
            verify_command,
            history: Vec::new(),
            failure_reason: None,
        }
    }

    /// Are all criteria graded and met?
    pub fn all_met(&self) -> bool {
        !self.contract.is_empty() && self.contract.iter().all(|c| c.met == Some(true))
    }

    /// How many criteria are currently graded as met.
    pub fn met_count(&self) -> u32 {
        self.contract.iter().filter(|c| c.met == Some(true)).count() as u32
    }

    /// The feature the epic is working on right now, if any.
    pub fn feature(&self) -> Option<&Feature> {
        self.features.get(self.current_feature)
    }

    /// Title of the current feature (empty string when there is no backlog yet).
    pub fn feature_title(&self) -> &str {
        self.feature().map(|f| f.title.as_str()).unwrap_or("")
    }

    /// A one-line overview of the backlog with the current feature marked, for role prompts.
    pub fn backlog_overview(&self) -> String {
        self.features
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let mark = if f.done {
                    "[x]"
                } else if i == self.current_feature {
                    "[>]"
                } else {
                    "[ ]"
                };
                format!("{mark} {}", f.title)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Record the decomposer's feature titles as the backlog and move to planning the first one.
pub fn set_features(state: &mut LoopState, titles: Vec<String>) {
    state.features = titles
        .into_iter()
        .map(|title| Feature { title, done: false })
        .collect();
    state.current_feature = 0;
    state.phase = LoopPhase::Planning;
}

/// Has the loop stalled — no upward progress over the last `window` rounds? True when the recent
/// window is flat (same met-count throughout) or its best does not exceed the best already seen
/// before it. Fewer than `window` graded rounds is never stuck (not enough evidence).
pub fn is_stuck(history: &[u32], window: usize) -> bool {
    let n = history.len();
    if window == 0 || n < window {
        return false;
    }
    let recent = &history[n - window..];
    let recent_flat = recent.iter().all(|&v| v == recent[0]);
    let recent_best = recent.iter().copied().max().unwrap_or(0);
    let prior_best = history[..n - window].iter().copied().max().unwrap_or(0);
    recent_flat || recent_best <= prior_best
}

/// Append a one-line round summary to `progress`, keeping only the last `max_lines` so the loop's
/// memory stays bounded (the generator/evaluator get a recent tail, not the whole history).
pub fn append_progress(progress: &mut String, line: &str, max_lines: usize) {
    if !progress.is_empty() {
        progress.push('\n');
    }
    progress.push_str(line);
    let lines: Vec<&str> = progress.lines().collect();
    if lines.len() > max_lines {
        *progress = lines[lines.len() - max_lines..].join("\n");
    }
}

// --- Role prompts (pure; the heart of role separation) ---

pub fn decomposer_prompt(spec: &str) -> String {
    format!(
        "You are the DECOMPOSER. You never write code and never plan criteria.\n\n\
         Break the request below into an ordered FEATURE LIST: the smallest sequence of \
         independently buildable, shippable increments that together deliver the whole request. \
         Order them so each builds on the last (foundations first). Aim for a handful to a dozen \
         features — not micro-tasks, not one giant blob.\n\n\
         Output a line containing only `FEATURES:` followed by a numbered list — one feature per \
         line as `N. <feature>`, and nothing else after the list. This exact shape is parsed \
         automatically.\n\n\
         REQUEST:\n{spec}\n"
    )
}

pub fn planner_prompt(spec: &str, feature: &str, backlog: &str) -> String {
    let context = if feature.is_empty() {
        String::new()
    } else {
        format!(
            "This is one feature of a larger build. FEATURE BACKLOG (\\[x]=done, \\[>]=current):\n\
             {backlog}\n\n\
             Plan ONLY the current feature — assume the done features already exist.\n\n\
             CURRENT FEATURE: {feature}\n\n"
        )
    };
    format!(
        "You are the PLANNER. You never write code.\n\n\
         {context}\
         Turn the feature below into a CONTRACT: a checklist of testable, unambiguous \"done\" \
         criteria (too few and the evaluator rubber-stamps). Each criterion must be objectively \
         checkable.\n\n\
         Output a line containing only `CONTRACT:` followed by the contract as a numbered list — \
         one criterion per line as `N. <criterion>`, and nothing else after the list. This exact \
         shape is parsed automatically.\n\n\
         OVERALL SPEC:\n{spec}\n"
    )
}

pub fn generator_prompt(
    spec: &str,
    feature: &str,
    contract: &[Criterion],
    progress: &str,
    verify_command: Option<&str>,
) -> String {
    let list = contract
        .iter()
        .enumerate()
        .map(|(i, c)| format!("{}. {}", i + 1, c.text))
        .collect::<Vec<_>>()
        .join("\n");
    let feature_line = if feature.is_empty() {
        String::new()
    } else {
        format!("CURRENT FEATURE (build only this; earlier features already exist): {feature}\n\n")
    };
    let mut memory = String::new();
    if !progress.trim().is_empty() {
        memory.push_str(&format!(
            "\nPROGRESS SO FAR (recent rounds — do NOT repeat what already failed; if attempts \
             have stalled, change your approach):\n{}\n",
            progress.trim()
        ));
    }
    if let Some(cmd) = verify_command {
        memory.push_str(&format!(
            "\nGROUND TRUTH: your work is accepted only when `{cmd}` exits 0. Run it and make it \
             pass before you finish.\n"
        ));
    }
    format!(
        "You are the GENERATOR. You write complete, working code. You are FORBIDDEN from \
         grading your own work.\n\n\
         {feature_line}\
         Implement so that every contract criterion is satisfied. No mocks, stubs, \
         TODOs, or placeholders. Commit your work when done.\n\n\
         OVERALL SPEC:\n{spec}\n\nCONTRACT:\n{list}\n{memory}"
    )
}

pub fn evaluator_prompt(
    feature: &str,
    contract: &[Criterion],
    diff: &str,
    verify_command: Option<&str>,
) -> String {
    let list = contract
        .iter()
        .enumerate()
        .map(|(i, c)| format!("{}. {}", i + 1, c.text))
        .collect::<Vec<_>>()
        .join("\n");
    let verify_line = match verify_command {
        Some(cmd) => format!(
            "Run the project's test command `{cmd}` and treat its result as ground truth: a \
             criterion the tests contradict, or that you cannot verify, is a FAIL.\n\n"
        ),
        None => String::new(),
    };
    let feature_line = if feature.is_empty() {
        String::new()
    } else {
        format!("You are grading only this feature: {feature}\n\n")
    };
    format!(
        "You are the EVALUATOR. Assume the code is BROKEN and your job is to prove it. You see \
         only the diff, not the author's reasoning. Run the tests, exercise the app.\n\n\
         {feature_line}\
         {verify_line}\
         For EACH criterion, output exactly one line `N. PASS` or `N. FAIL: <evidence>`, using \
         the criterion's number below, then a short evidence paragraph. Do not give the \
         benefit of the doubt; a criterion you cannot verify is a FAIL. The `N. PASS/FAIL` \
         lines are parsed automatically.\n\n\
         CONTRACT:\n{list}\n\nDIFF:\n{diff}\n"
    )
}

// --- Output parsing (auto-advance: turn a role agent's terminal output into state) ---

/// Strip terminal escape sequences and control noise so a role agent's scrollback can be
/// parsed as plain text. Handles CSI (`ESC [ … final`) and OSC (`ESC ] … BEL/ST`) sequences,
/// collapses carriage-return redraws to the final overwrite per line, keeps `\n`/`\t`, and
/// drops other C0 control bytes.
pub fn strip_ansi(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\u{1b}' {
            match chars.get(i + 1) {
                // CSI: ESC [ ... <final byte 0x40..=0x7e>
                Some('[') => {
                    i += 2;
                    while i < chars.len() && !matches!(chars[i], '\u{40}'..='\u{7e}') {
                        i += 1;
                    }
                    i += 1; // consume the final byte
                }
                // OSC: ESC ] ... terminated by BEL or ST (ESC \)
                Some(']') => {
                    i += 2;
                    while i < chars.len() {
                        if chars[i] == '\u{07}' {
                            i += 1;
                            break;
                        }
                        if chars[i] == '\u{1b}' && chars.get(i + 1) == Some(&'\\') {
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                }
                // Any other two-char escape (charset select, etc.).
                Some(_) => i += 2,
                None => i += 1,
            }
            continue;
        }
        out.push(c);
        i += 1;
    }
    // Apply carriage-return overwrites per line, then drop remaining control bytes.
    out.split('\n')
        .map(|line| {
            let visible = line.rsplit('\r').next().unwrap_or(line);
            visible
                .chars()
                .filter(|c| *c == '\t' || !c.is_control())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// If `line` is a list item (numbered `1.`/`1)` or a `-`/`*`/`+` bullet, optionally with a
/// `[ ]`/`[x]` checkbox), return its text. Otherwise `None`.
fn list_item_text(line: &str) -> Option<String> {
    let t = line.trim_start();
    let rest = if let Some(r) = t
        .strip_prefix("- ")
        .or_else(|| t.strip_prefix("* "))
        .or_else(|| t.strip_prefix("+ "))
    {
        r
    } else {
        // Numbered: digits then '.' or ')' then whitespace.
        let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            return None;
        }
        let after = &t[digits.len()..];
        let sep = after.chars().next()?;
        if sep != '.' && sep != ')' {
            return None;
        }
        after[1..].trim_start()
    };
    // Drop a leading markdown checkbox if present.
    let rest = rest
        .strip_prefix("[ ] ")
        .or_else(|| rest.strip_prefix("[x] "))
        .or_else(|| rest.strip_prefix("[X] "))
        .unwrap_or(rest);
    let text = rest.trim().trim_matches('*').trim_matches('`').trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

/// Extract a numbered/bulleted list from a role agent's output: the items following a `header`
/// line (e.g. `CONTRACT` / `FEATURES`), or — absent that header — every list item in the output.
pub fn parse_list(output: &str, header: &str) -> Vec<String> {
    let clean = strip_ansi(output);
    let lines: Vec<&str> = clean.lines().collect();
    let header_up = header.to_uppercase();
    let start = lines
        .iter()
        .position(|l| {
            let u = l.trim().to_uppercase();
            u.contains(&header_up) && list_item_text(l).is_none()
        })
        .map(|i| i + 1)
        .unwrap_or(0);
    lines[start..]
        .iter()
        .filter_map(|l| list_item_text(l))
        .collect()
}

/// The contract criteria from the planner's output (items after a `CONTRACT` header).
pub fn parse_contract(output: &str) -> Vec<String> {
    parse_list(output, "CONTRACT")
}

/// The feature backlog from the decomposer's output (items after a `FEATURES` header).
pub fn parse_features(output: &str) -> Vec<String> {
    parse_list(output, "FEATURES")
}

/// Return `true` iff `line` reports a verdict for criterion number `n`, and give it. Accepts
/// `n. PASS`, `n) FAIL: …`, and `Criterion n: PASS`. FAIL wins over PASS on the same line.
fn verdict_on_line(line: &str, n: usize) -> Option<bool> {
    let mut t = line.trim_start();
    // Optional leading bullet / label word before the number.
    for pfx in ["- ", "* ", "+ "] {
        if let Some(r) = t.strip_prefix(pfx) {
            t = r.trim_start();
        }
    }
    for word in ["criterion", "item", "check", "#"] {
        if t.to_lowercase().starts_with(word) {
            t = t[word.len()..].trim_start();
        }
    }
    let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() || digits.parse::<usize>().ok()? != n {
        return None;
    }
    let after = &t[digits.len()..];
    // Require a separator so "1" does not match inside "10".
    let sep = after.chars().next()?;
    if !matches!(sep, '.' | ')' | ':' | ' ' | '-') {
        return None;
    }
    let up = after.to_uppercase();
    if up.contains("FAIL") {
        Some(false)
    } else if up.contains("PASS") {
        Some(true)
    } else {
        None
    }
}

/// Map the evaluator's output to a per-criterion verdict, in contract order. A criterion with
/// no clear PASS line is treated as failed — the evaluator assumes the code is broken.
pub fn parse_verdicts(output: &str, contract: &[Criterion]) -> Vec<bool> {
    let clean = strip_ansi(output);
    let lines: Vec<&str> = clean.lines().collect();
    (1..=contract.len())
        .map(|n| {
            lines
                .iter()
                .find_map(|l| verdict_on_line(l, n))
                .unwrap_or(false)
        })
        .collect()
}

/// The prompt to run for the loop's current phase, if a role is due.
pub fn prompt_for_phase(state: &LoopState, diff: &str) -> Option<(Role, String)> {
    match state.phase {
        LoopPhase::Decomposing => Some((Role::Decomposer, decomposer_prompt(&state.spec))),
        LoopPhase::Planning => Some((
            Role::Planner,
            planner_prompt(
                &state.spec,
                state.feature_title(),
                &state.backlog_overview(),
            ),
        )),
        LoopPhase::Generating => Some((
            Role::Generator,
            generator_prompt(
                &state.spec,
                state.feature_title(),
                &state.contract,
                &state.progress,
                state.verify_command.as_deref(),
            ),
        )),
        LoopPhase::Evaluating => Some((
            Role::Evaluator,
            evaluator_prompt(
                state.feature_title(),
                &state.contract,
                diff,
                state.verify_command.as_deref(),
            ),
        )),
        LoopPhase::Passed | LoopPhase::Failed => None,
    }
}

/// Apply the evaluator's per-criterion verdicts plus the ground-truth test result, and advance.
///
/// `verify` is the exit-status verdict of the loop's verify command: `Some(true)` = tests pass,
/// `Some(false)` = tests fail, `None` = no verify command configured. The loop reaches `Passed`
/// only when every criterion is met AND the tests did not fail — so failing tests block a pass
/// even if the evaluator rated every criterion PASS. Otherwise the loop retries, unless it has
/// stalled (no progress in `STUCK_WINDOW` rounds) or run out of iterations, either of which ends
/// it as `Failed` with a recorded reason (LOOPS XXXI).
pub fn grade_and_advance(state: &mut LoopState, verdicts: &[bool], verify: Option<bool>) {
    for (c, met) in state.contract.iter_mut().zip(verdicts.iter()) {
        c.met = Some(*met);
    }
    state.history.push(state.met_count());
    let met = state.met_count();
    let total = state.contract.len() as u32;

    // Feature passes: its contract is met and the tests did not fail.
    if state.all_met() && verify != Some(false) {
        advance_feature(state);
        return;
    }

    // Fail-fast: a stalled or exhausted feature ends the whole epic, naming the feature.
    let tests_note = if verify == Some(false) {
        "; tests failing"
    } else {
        ""
    };
    let feature_note = match state.feature() {
        Some(f) => format!(" on feature \"{}\"", f.title),
        None => String::new(),
    };
    if is_stuck(&state.history, STUCK_WINDOW) {
        state.phase = LoopPhase::Failed;
        state.failure_reason = Some(format!(
            "no progress in {STUCK_WINDOW} rounds{feature_note} ({met}/{total} criteria met{tests_note})"
        ));
    } else if state.iteration + 1 >= state.max_iterations {
        state.phase = LoopPhase::Failed;
        state.failure_reason = Some(format!(
            "out of iterations{feature_note} ({met}/{total} criteria met{tests_note})"
        ));
    } else {
        state.iteration += 1;
        state.phase = LoopPhase::Generating;
        state.failure_reason = None;
    }
}

/// A feature's contract is satisfied: mark it done and either move on to plan the next feature
/// (resetting the per-feature round state) or, if the backlog is exhausted, complete the epic.
/// With no backlog at all (a single ad-hoc contract), a met contract simply passes.
fn advance_feature(state: &mut LoopState) {
    state.failure_reason = None;
    if let Some(f) = state.features.get_mut(state.current_feature) {
        f.done = true;
    }
    let next = state.current_feature + 1;
    if next < state.features.len() {
        append_progress(
            &mut state.progress,
            &format!(
                "feature \"{}\" done ({}/{})",
                state.features[state.current_feature].title,
                next,
                state.features.len()
            ),
            15,
        );
        state.current_feature = next;
        state.contract = Vec::new();
        state.iteration = 0;
        state.history = Vec::new();
        state.base_commit = None;
        state.phase = LoopPhase::Planning;
    } else {
        state.phase = LoopPhase::Passed;
    }
}

// --- Disk state (LOOPS XXX: state lives in files, not context) ---

fn loop_dir(base: &Path, id: &str) -> PathBuf {
    base.join(id)
}

/// Persist the full loop state plus the human-facing companion files.
pub fn save(base: &Path, state: &LoopState) -> AppResult<()> {
    let dir = loop_dir(base, &state.id);
    fs::create_dir_all(&dir)?;
    fs::write(dir.join("state.json"), serde_json::to_string_pretty(state)?)?;
    fs::write(
        dir.join("feature-list.json"),
        serde_json::to_string_pretty(&state.features)?,
    )?;
    let contract_md = state
        .contract
        .iter()
        .map(|c| {
            let box_ = match c.met {
                Some(true) => "[x]",
                Some(false) => "[ ]",
                None => "[ ]",
            };
            format!("- {} {}", box_, c.text)
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        dir.join("contract.md"),
        format!("# Contract\n\n{contract_md}\n"),
    )?;
    fs::write(dir.join("progress.md"), &state.progress)?;
    Ok(())
}

pub fn load(base: &Path, id: &str) -> AppResult<LoopState> {
    let raw = fs::read_to_string(loop_dir(base, id).join("state.json"))
        .map_err(|_| AppError::NotFound(format!("loop {id}")))?;
    Ok(serde_json::from_str(&raw)?)
}

/// Append a timestamped-by-caller line to the loop's append-only log.
pub fn append_log(base: &Path, id: &str, line: &str) -> AppResult<()> {
    let dir = loop_dir(base, id);
    fs::create_dir_all(&dir)?;
    let mut existing = fs::read_to_string(dir.join("log.md")).unwrap_or_default();
    existing.push_str(line);
    existing.push('\n');
    fs::write(dir.join("log.md"), existing)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "autodev-loop-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn crit(text: &str) -> Criterion {
        Criterion {
            text: text.to_string(),
            met: None,
        }
    }

    fn mk(id: &str) -> LoopState {
        LoopState::with_options(
            id.into(),
            "spec".into(),
            "/p".into(),
            None,
            DEFAULT_MAX_ITERATIONS,
        )
    }

    #[test]
    fn role_prompts_carry_their_role_and_inputs() {
        assert!(decomposer_prompt("build X").contains("You are the DECOMPOSER"));
        assert!(decomposer_prompt("build X").contains("FEATURES:"));
        assert!(planner_prompt("build X", "", "").contains("You are the PLANNER"));
        assert!(planner_prompt("build X", "", "").contains("build X"));
        let contract = vec![crit("has a login")];
        let gen = generator_prompt("spec", "", &contract, "", None);
        assert!(gen.contains("FORBIDDEN from grading"));
        assert!(gen.contains("has a login"));
        let ev = evaluator_prompt("", &contract, "the diff", None);
        assert!(ev.contains("code is BROKEN"));
        assert!(ev.contains("the diff"));
    }

    #[test]
    fn feature_context_threads_into_the_prompts() {
        assert!(planner_prompt("spec", "user login", "[>] user login").contains("user login"));
        assert!(planner_prompt("spec", "user login", "").contains("CURRENT FEATURE: user login"));
        let contract = vec![crit("c")];
        assert!(generator_prompt("spec", "user login", &contract, "", None).contains("user login"));
        assert!(evaluator_prompt("user login", &contract, "d", None).contains("user login"));
    }

    #[test]
    fn prompts_carry_progress_memory_and_the_verify_command() {
        let contract = vec![crit("adds two numbers")];
        let gen = generator_prompt(
            "spec",
            "",
            &contract,
            "iteration 1: 0/1 met; failing: adds two numbers",
            Some("npm test"),
        );
        assert!(gen.contains("PROGRESS SO FAR"));
        assert!(gen.contains("failing: adds two numbers"));
        assert!(gen.contains("npm test"));
        // No memory when there's nothing to carry.
        assert!(!generator_prompt("spec", "", &contract, "", None).contains("PROGRESS SO FAR"));
        assert!(
            evaluator_prompt("", &contract, "d", Some("./dev.sh verify"))
                .contains("./dev.sh verify")
        );
    }

    #[test]
    fn prompt_for_phase_follows_the_state() {
        let mut s = mk("l1");
        assert_eq!(prompt_for_phase(&s, "").unwrap().0, Role::Decomposer);
        s.phase = LoopPhase::Planning;
        assert_eq!(prompt_for_phase(&s, "").unwrap().0, Role::Planner);
        s.phase = LoopPhase::Generating;
        assert_eq!(prompt_for_phase(&s, "").unwrap().0, Role::Generator);
        s.phase = LoopPhase::Evaluating;
        assert_eq!(prompt_for_phase(&s, "").unwrap().0, Role::Evaluator);
        s.phase = LoopPhase::Passed;
        assert!(prompt_for_phase(&s, "").is_none());
    }

    #[test]
    fn set_features_starts_the_backlog_at_planning() {
        let mut s = mk("l");
        set_features(&mut s, vec!["auth".into(), "posts".into(), "search".into()]);
        assert_eq!(s.phase, LoopPhase::Planning);
        assert_eq!(s.current_feature, 0);
        assert_eq!(s.feature_title(), "auth");
        assert!(s.backlog_overview().contains("[>] auth"));
        assert!(s.backlog_overview().contains("[ ] posts"));
    }

    #[test]
    fn parse_features_reads_the_backlog() {
        let out =
            "Here is the plan.\nFEATURES:\n1. user auth\n2. create posts\n3) full-text search\n";
        assert_eq!(
            parse_features(out),
            vec!["user auth", "create posts", "full-text search"]
        );
    }

    #[test]
    fn a_passing_feature_advances_to_the_next_then_completes_the_epic() {
        let mut s = mk("l");
        set_features(&mut s, vec!["auth".into(), "posts".into()]);
        // Build feature 0.
        s.contract = vec![crit("login works")];
        s.phase = LoopPhase::Evaluating;
        grade_and_advance(&mut s, &[true], Some(true));
        // Not done — moved on to plan feature 1, with per-feature state reset.
        assert_eq!(s.phase, LoopPhase::Planning);
        assert_eq!(s.current_feature, 1);
        assert!(s.features[0].done);
        assert!(s.contract.is_empty());
        assert_eq!(s.iteration, 0);
        assert!(s.progress.contains("feature \"auth\" done (1/2)"));
        // Build feature 1 → epic complete.
        s.contract = vec![crit("posts render")];
        s.phase = LoopPhase::Evaluating;
        grade_and_advance(&mut s, &[true], Some(true));
        assert_eq!(s.phase, LoopPhase::Passed);
        assert!(s.features.iter().all(|f| f.done));
    }

    #[test]
    fn a_failed_feature_fails_the_epic_naming_the_feature() {
        let mut s = mk("l");
        set_features(&mut s, vec!["auth".into(), "posts".into()]);
        s.max_iterations = 1; // fail feature 0 immediately
        s.contract = vec![crit("login works")];
        s.phase = LoopPhase::Evaluating;
        grade_and_advance(&mut s, &[false], None);
        assert_eq!(s.phase, LoopPhase::Failed);
        assert!(s.failure_reason.as_deref().unwrap().contains("auth"));
        assert!(!s.features[1].done); // never reached feature 1
    }

    #[test]
    fn all_pass_moves_to_passed() {
        let mut s = mk("l");
        s.contract = vec![crit("a"), crit("b")];
        s.phase = LoopPhase::Evaluating;
        grade_and_advance(&mut s, &[true, true], None);
        assert_eq!(s.phase, LoopPhase::Passed);
        assert!(s.all_met());
    }

    #[test]
    fn a_failure_retries_until_max_then_fails_with_a_reason() {
        let mut s = mk("l");
        s.contract = vec![crit("a"), crit("b"), crit("c")];
        s.max_iterations = 2;
        s.phase = LoopPhase::Evaluating;
        // Progress each round (1 met, then 2 met) so it is not flagged as stuck.
        grade_and_advance(&mut s, &[true, false, false], None);
        assert_eq!(s.phase, LoopPhase::Generating); // iteration 0 -> retry
        assert_eq!(s.iteration, 1);
        s.phase = LoopPhase::Evaluating;
        grade_and_advance(&mut s, &[true, true, false], None);
        assert_eq!(s.phase, LoopPhase::Failed); // hit max_iterations
        assert!(s
            .failure_reason
            .as_deref()
            .unwrap()
            .contains("out of iterations"));
        assert!(s.failure_reason.as_deref().unwrap().contains("2/3"));
    }

    #[test]
    fn failing_tests_block_a_pass_even_when_every_verdict_is_pass() {
        let mut s = mk("l");
        s.contract = vec![crit("a"), crit("b")];
        s.phase = LoopPhase::Evaluating;
        // Evaluator says everything passes, but ground-truth tests fail: must NOT pass.
        grade_and_advance(&mut s, &[true, true], Some(false));
        assert_eq!(s.phase, LoopPhase::Generating);
        // Same verdicts, tests now pass: passes.
        s.phase = LoopPhase::Evaluating;
        grade_and_advance(&mut s, &[true, true], Some(true));
        assert_eq!(s.phase, LoopPhase::Passed);
    }

    #[test]
    fn a_stalled_loop_fails_early_as_stuck() {
        let mut s = mk("l");
        s.contract = vec![crit("a"), crit("b"), crit("c")];
        s.max_iterations = 20; // far from the cap; stuck-detection must end it first
        for _ in 0..STUCK_WINDOW {
            s.phase = LoopPhase::Evaluating;
            grade_and_advance(&mut s, &[true, false, false], None); // stuck at 1/3
        }
        assert_eq!(s.phase, LoopPhase::Failed);
        assert!(s.failure_reason.as_deref().unwrap().contains("no progress"));
        assert!(s.iteration < 20);
    }

    #[test]
    fn is_stuck_needs_a_plateau() {
        assert!(!is_stuck(&[], STUCK_WINDOW));
        assert!(!is_stuck(&[1, 2], STUCK_WINDOW)); // too few rounds
        assert!(!is_stuck(&[1, 2, 3], STUCK_WINDOW)); // improving
        assert!(!is_stuck(&[0, 1, 2, 3], STUCK_WINDOW)); // still improving in the window
        assert!(is_stuck(&[3, 3, 3], STUCK_WINDOW)); // flat
        assert!(is_stuck(&[2, 3, 2, 3, 3], STUCK_WINDOW)); // no new best in last 3
    }

    #[test]
    fn append_progress_keeps_a_bounded_tail() {
        let mut p = String::new();
        for i in 1..=10 {
            append_progress(&mut p, &format!("iteration {i}"), 3);
        }
        let lines: Vec<&str> = p.lines().collect();
        assert_eq!(lines, vec!["iteration 8", "iteration 9", "iteration 10"]);
    }

    #[test]
    fn with_options_sets_verify_and_cap() {
        let s = LoopState::with_options(
            "l".into(),
            "spec".into(),
            "/p".into(),
            Some("npm test".into()),
            12,
        );
        assert_eq!(s.verify_command.as_deref(), Some("npm test"));
        assert_eq!(s.max_iterations, 12);
        // Cap is floored at 1 so a loop always runs at least once.
        assert_eq!(
            LoopState::with_options("l".into(), "s".into(), "/p".into(), None, 0).max_iterations,
            1
        );
    }

    #[test]
    fn strip_ansi_removes_csi_and_osc() {
        assert_eq!(strip_ansi("\x1b[31mred\x1b[0m text"), "red text");
        // OSC window-title set, terminated by BEL, then real text.
        assert_eq!(strip_ansi("\x1b]0;a title\x07hello"), "hello");
        // A carriage-return redraw keeps only the final overwrite on the line.
        assert_eq!(strip_ansi("loading...\rdone\n"), "done\n");
        // Lone control bytes are dropped; newline and tab survive.
        assert_eq!(strip_ansi("a\x08b\tc\n"), "ab\tc\n");
    }

    #[test]
    fn parse_contract_reads_the_numbered_list_after_the_header() {
        let out = "\
Here is the spec. It has 1 goal, described below.\n\
\n\
CONTRACT:\n\
1. Rejects a missing email with 400\n\
2. Short code is exactly 6 chars\n\
3) Redirect returns 302\n\
- [ ] Has an index page\n\
\n\
That is the contract.\n";
        assert_eq!(
            parse_contract(out),
            vec![
                "Rejects a missing email with 400",
                "Short code is exactly 6 chars",
                "Redirect returns 302",
                "Has an index page",
            ]
        );
    }

    #[test]
    fn parse_contract_falls_back_to_all_numbered_lines_without_a_header() {
        assert_eq!(parse_contract("1. alpha\n2. beta\n"), vec!["alpha", "beta"]);
    }

    #[test]
    fn parse_contract_survives_ansi_colouring() {
        let out = "CONTRACT:\n\x1b[32m1. green criterion\x1b[0m\n";
        assert_eq!(parse_contract(out), vec!["green criterion"]);
    }

    #[test]
    fn parse_verdicts_maps_pass_fail_per_numbered_criterion() {
        let contract = vec![crit("a"), crit("b"), crit("c")];
        let out = "\
1. PASS — verified by running the test\n\
2. FAIL: returns 200 instead of 400\n\
Criterion 3: PASS\n";
        assert_eq!(parse_verdicts(out, &contract), vec![true, false, true]);
    }

    #[test]
    fn parse_verdicts_defaults_missing_criteria_to_fail() {
        let contract = vec![crit("a"), crit("b")];
        // Only criterion 1 is reported; the evaluator is adversarial, so an unreported
        // criterion is a failure, not a pass.
        assert_eq!(parse_verdicts("1. PASS\n", &contract), vec![true, false]);
    }

    #[test]
    fn parse_verdicts_length_matches_the_contract() {
        let contract = vec![crit("a"), crit("b"), crit("c")];
        assert_eq!(parse_verdicts("nonsense output", &contract).len(), 3);
    }

    #[test]
    fn disk_roundtrip_and_companion_files() {
        let base = tmp("disk");
        let mut s = LoopState::with_options("loop1".into(), "spec".into(), "/proj".into(), None, 8);
        s.contract = vec![Criterion {
            text: "has tests".into(),
            met: Some(true),
        }];
        s.features = vec![Feature {
            title: "feature a".into(),
            done: false,
        }];
        s.progress = "did the thing".into();
        save(&base, &s).unwrap();
        append_log(&base, "loop1", "started").unwrap();

        let loaded = load(&base, "loop1").unwrap();
        assert_eq!(loaded, s);
        let contract_md = fs::read_to_string(base.join("loop1/contract.md")).unwrap();
        assert!(contract_md.contains("[x] has tests"));
        assert!(fs::read_to_string(base.join("loop1/log.md"))
            .unwrap()
            .contains("started"));
        let _ = fs::remove_dir_all(&base);
    }
}
