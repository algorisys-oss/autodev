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
         Output the spec, then the contract as a numbered list.\n\n\
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
         For EACH criterion, output PASS or FAIL with concrete evidence. Do not give the \
         benefit of the doubt.\n\n\
         CONTRACT:\n{list}\n\nDIFF:\n{diff}\n"
    )
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
