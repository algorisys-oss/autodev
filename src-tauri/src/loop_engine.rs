use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

/// The three roles never share a context (LOOPS XXVIII). Each has its own system prompt.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Planner,
    Generator,
    Evaluator,
}

/// Where a loop is in its life cycle.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoopPhase {
    /// Planner is turning the spec into a contract.
    Planning,
    /// Generator is implementing against the contract.
    Generating,
    /// Evaluator is grading the diff against the contract.
    Evaluating,
    /// Every criterion met.
    Passed,
    /// Out of iterations without meeting the contract.
    Failed,
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
    pub features: Vec<String>,
    pub progress: String,
}

impl LoopState {
    pub fn new(id: String, spec: String, project_dir: String) -> Self {
        Self {
            id,
            spec,
            project_dir,
            phase: LoopPhase::Planning,
            iteration: 0,
            max_iterations: 5,
            contract: Vec::new(),
            features: Vec::new(),
            progress: String::new(),
        }
    }

    /// Are all criteria graded and met?
    pub fn all_met(&self) -> bool {
        !self.contract.is_empty() && self.contract.iter().all(|c| c.met == Some(true))
    }
}

// --- Role prompts (pure; the heart of role separation) ---

pub fn planner_prompt(spec: &str) -> String {
    format!(
        "You are the PLANNER. You never write code.\n\n\
         Turn the request below into a concrete spec, then a CONTRACT: a checklist of \
         testable, unambiguous \"done\" criteria (aim for 20+ for a small app; too few and \
         the evaluator rubber-stamps). Each criterion must be objectively checkable.\n\n\
         Output the spec first. Then output a line containing only `CONTRACT:` followed by \
         the contract as a numbered list — one criterion per line as `N. <criterion>`, and \
         nothing else after the list. This exact shape is parsed automatically.\n\n\
         REQUEST:\n{spec}\n"
    )
}

pub fn generator_prompt(spec: &str, contract: &[Criterion]) -> String {
    let list = contract
        .iter()
        .enumerate()
        .map(|(i, c)| format!("{}. {}", i + 1, c.text))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "You are the GENERATOR. You write complete, working code. You are FORBIDDEN from \
         grading your own work.\n\n\
         Implement the spec so that every contract criterion is satisfied. No mocks, stubs, \
         TODOs, or placeholders. Commit your work when done.\n\n\
         SPEC:\n{spec}\n\nCONTRACT:\n{list}\n"
    )
}

pub fn evaluator_prompt(contract: &[Criterion], diff: &str) -> String {
    let list = contract
        .iter()
        .enumerate()
        .map(|(i, c)| format!("{}. {}", i + 1, c.text))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "You are the EVALUATOR. Assume the code is BROKEN and your job is to prove it. You see \
         only the diff, not the author's reasoning. Run the tests, exercise the app.\n\n\
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

/// Extract the contract criteria from the planner's output: the list items following a
/// `CONTRACT` header, or — absent a header — every list item in the output.
pub fn parse_contract(output: &str) -> Vec<String> {
    let clean = strip_ansi(output);
    let lines: Vec<&str> = clean.lines().collect();
    let start = lines
        .iter()
        .position(|l| {
            let u = l.trim().to_uppercase();
            u.contains("CONTRACT") && list_item_text(l).is_none()
        })
        .map(|i| i + 1)
        .unwrap_or(0);
    lines[start..]
        .iter()
        .filter_map(|l| list_item_text(l))
        .collect()
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
        LoopPhase::Planning => Some((Role::Planner, planner_prompt(&state.spec))),
        LoopPhase::Generating => Some((
            Role::Generator,
            generator_prompt(&state.spec, &state.contract),
        )),
        LoopPhase::Evaluating => Some((Role::Evaluator, evaluator_prompt(&state.contract, diff))),
        LoopPhase::Passed | LoopPhase::Failed => None,
    }
}

/// Apply the evaluator's per-criterion verdicts and advance the phase.
///
/// All met → Passed. Otherwise, if iterations remain, reset the failed marks and go back to
/// Generating (LOOPS XXXI: let the loop retry). Out of iterations → Failed.
pub fn grade_and_advance(state: &mut LoopState, verdicts: &[bool]) {
    for (c, met) in state.contract.iter_mut().zip(verdicts.iter()) {
        c.met = Some(*met);
    }
    if state.all_met() {
        state.phase = LoopPhase::Passed;
    } else if state.iteration + 1 >= state.max_iterations {
        state.phase = LoopPhase::Failed;
    } else {
        state.iteration += 1;
        state.phase = LoopPhase::Generating;
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

    #[test]
    fn role_prompts_carry_their_role_and_inputs() {
        assert!(planner_prompt("build X").contains("You are the PLANNER"));
        assert!(planner_prompt("build X").contains("build X"));
        let contract = vec![crit("has a login")];
        assert!(generator_prompt("spec", &contract).contains("FORBIDDEN from grading"));
        assert!(generator_prompt("spec", &contract).contains("has a login"));
        assert!(evaluator_prompt(&contract, "the diff").contains("code is BROKEN"));
        assert!(evaluator_prompt(&contract, "the diff").contains("the diff"));
    }

    #[test]
    fn prompt_for_phase_follows_the_state() {
        let mut s = LoopState::new("l1".into(), "spec".into(), "/p".into());
        assert_eq!(prompt_for_phase(&s, "").unwrap().0, Role::Planner);
        s.phase = LoopPhase::Generating;
        assert_eq!(prompt_for_phase(&s, "").unwrap().0, Role::Generator);
        s.phase = LoopPhase::Evaluating;
        assert_eq!(prompt_for_phase(&s, "").unwrap().0, Role::Evaluator);
        s.phase = LoopPhase::Passed;
        assert!(prompt_for_phase(&s, "").is_none());
    }

    #[test]
    fn all_pass_moves_to_passed() {
        let mut s = LoopState::new("l".into(), "spec".into(), "/p".into());
        s.contract = vec![crit("a"), crit("b")];
        s.phase = LoopPhase::Evaluating;
        grade_and_advance(&mut s, &[true, true]);
        assert_eq!(s.phase, LoopPhase::Passed);
        assert!(s.all_met());
    }

    #[test]
    fn a_failure_retries_until_max_then_fails() {
        let mut s = LoopState::new("l".into(), "spec".into(), "/p".into());
        s.contract = vec![crit("a"), crit("b")];
        s.max_iterations = 2;
        s.phase = LoopPhase::Evaluating;
        grade_and_advance(&mut s, &[true, false]);
        assert_eq!(s.phase, LoopPhase::Generating); // iteration 0 -> retry
        assert_eq!(s.iteration, 1);
        s.phase = LoopPhase::Evaluating;
        grade_and_advance(&mut s, &[true, false]);
        assert_eq!(s.phase, LoopPhase::Failed); // hit max_iterations
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
        let mut s = LoopState::new("loop1".into(), "spec".into(), "/proj".into());
        s.contract = vec![Criterion {
            text: "has tests".into(),
            met: Some(true),
        }];
        s.features = vec!["feature a".into()];
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
