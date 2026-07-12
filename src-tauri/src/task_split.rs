//! Pre-launch task splitter: decide whether a request parallelizes across independent
//! agents, infer its difficulty, and emit one self-contained sub-prompt per unit.
//!
//! This is stateless and pure (prompt construction + output parsing). The classifier is a
//! one-shot agent run *before* launch; its terminal output is parsed back here into a
//! `TaskPlan` the composer applies to its existing per-agent fan-out. Unlike the loop
//! engine's decomposer (an ordered, serial backlog), the units here are meant to run at the
//! same time.

use serde::{Deserialize, Serialize};

/// Hard cap on parallel units accepted from the classifier — matches the decomposer's
/// "handful to a dozen" guidance and keeps a runaway split reviewable by a human.
pub const MAX_UNITS: usize = 12;

/// One independently-runnable slice of a task, launched as its own agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskUnit {
    /// Short label for the agent card.
    pub title: String,
    /// A complete, self-contained instruction — the agent sees only this.
    pub prompt: String,
    /// Project `@mentions` this unit needs as extra context (may be empty).
    #[serde(default)]
    pub mentions: Vec<String>,
}

/// The classifier's verdict on how to run a task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskPlan {
    /// Inferred 1..10 difficulty (feeds the existing difficulty heuristic).
    pub difficulty: u8,
    /// Whether the units are independent enough to run in parallel. Always `false` when
    /// there is only one unit.
    pub parallel: bool,
    pub units: Vec<TaskUnit>,
    /// One-line justification, shown to the user reviewing the split.
    pub rationale: String,
}

/// Build the classifier prompt for `task`. `projects` are the workspace project names the
/// classifier may reference in a unit's `mentions`.
pub fn split_prompt(task: &str, projects: &[String]) -> String {
    let project_line = if projects.is_empty() {
        String::new()
    } else {
        format!(
            "Available project @mentions for the `mentions` field: {}.\n\n",
            projects.join(", ")
        )
    };
    format!(
        "You are the TASK SPLITTER. You never write code and never change anything. You decide \
         how to PARALLELIZE the request below across independent coding agents.\n\n\
         A task is PARALLELIZABLE only if it breaks into units that can run at the SAME TIME \
         without depending on each other's output — e.g. process each of many files, apply the \
         same change across unrelated modules, generate N independent artifacts. A task is NOT \
         parallelizable if the steps must happen in order or share state (a single feature, a \
         refactor threaded through one call path, a bug fix).\n\n\
         You MAY inspect the working directory (READ-ONLY) to enumerate concrete work items — \
         list files, read a manifest — so \"convert every video in ./media\" becomes one unit per \
         real file. Do NOT modify anything.\n\n\
         {project_line}\
         Also rate DIFFICULTY from 1 (trivial one-line edit) to 10 (large, multi-part, needs \
         planning).\n\n\
         Finish your reply with ONLY this block and nothing after it:\n\n\
         <<<TASKPLAN\n\
         {{\"difficulty\": <1-10>, \"parallel\": <true|false>, \"units\": [{{\"title\": \"short \
         label\", \"prompt\": \"a complete, self-contained instruction for one agent\", \
         \"mentions\": [\"project\"]}}], \"rationale\": \"one sentence\"}}\n\
         TASKPLAN\n\n\
         If it is NOT parallelizable, set \"parallel\": false and return exactly one unit whose \
         prompt is the full task. Use at most {MAX_UNITS} units. Each unit's prompt must stand \
         alone — an agent sees only its own prompt, never the others.\n\n\
         REQUEST:\n{task}\n"
    )
}

/// Lenient shape for deserialization: tolerate an integer or float difficulty and missing
/// fields, then normalize/validate into a `TaskPlan`.
#[derive(Deserialize)]
struct RawPlan {
    #[serde(default)]
    difficulty: f64,
    #[serde(default)]
    parallel: bool,
    #[serde(default)]
    units: Vec<RawUnit>,
    #[serde(default)]
    rationale: String,
}

#[derive(Deserialize)]
struct RawUnit {
    #[serde(default)]
    title: String,
    #[serde(default)]
    prompt: String,
    #[serde(default)]
    mentions: Vec<String>,
}

/// Parse a classifier agent's terminal output into a `TaskPlan`, or `None` if no valid plan
/// block is present. Strips terminal escapes, takes the LAST `<<<TASKPLAN … TASKPLAN` block
/// (the model's final answer), deserializes the JSON, and validates it.
pub fn parse_task_plan(output: &str) -> Option<TaskPlan> {
    let clean = crate::loop_engine::strip_ansi(output);
    const OPEN: &str = "<<<TASKPLAN";
    const CLOSE: &str = "TASKPLAN";
    let open = clean.rfind(OPEN)?;
    let after = &clean[open + OPEN.len()..];
    let close = after.find(CLOSE)?;
    let json = after[..close].trim();
    let raw: RawPlan = serde_json::from_str(json).ok()?;
    normalize(raw)
}

/// Clamp difficulty to 1..10, drop empty units, cap at `MAX_UNITS`, and force `parallel`
/// off when only one unit survives. Returns `None` if no usable unit remains.
fn normalize(raw: RawPlan) -> Option<TaskPlan> {
    let difficulty = raw.difficulty.round().clamp(1.0, 10.0) as u8;
    let mut units: Vec<TaskUnit> = raw
        .units
        .into_iter()
        .filter_map(|u| {
            let prompt = u.prompt.trim().to_string();
            if prompt.is_empty() {
                return None;
            }
            let title = u.title.trim();
            let title = if title.is_empty() {
                prompt.clone()
            } else {
                title.to_string()
            };
            let mentions = u
                .mentions
                .into_iter()
                .map(|m| m.trim().to_string())
                .filter(|m| !m.is_empty())
                .collect();
            Some(TaskUnit {
                title,
                prompt,
                mentions,
            })
        })
        .collect();
    if units.is_empty() {
        return None;
    }
    units.truncate(MAX_UNITS);
    // Only fan out when there is genuinely more than one independent unit. A non-parallel
    // verdict collapses to a single agent running the (full) first unit.
    let parallel = raw.parallel && units.len() > 1;
    if !parallel {
        units.truncate(1);
    }
    Some(TaskPlan {
        difficulty,
        parallel,
        units,
        rationale: raw.rationale.trim().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_prompt_carries_task_and_contract() {
        let p = split_prompt("transcode every video in ./media", &[]);
        assert!(p.contains("TASK SPLITTER"));
        assert!(p.contains("transcode every video in ./media"));
        assert!(p.contains("<<<TASKPLAN"));
        assert!(p.contains("PARALLELIZABLE"));
        // Read-only guardrail is stated.
        assert!(p.contains("READ-ONLY"));
    }

    #[test]
    fn split_prompt_lists_projects_when_present() {
        let p = split_prompt("do a thing", &["api".into(), "web-ui".into()]);
        assert!(p.contains("api, web-ui"));
        let none = split_prompt("do a thing", &[]);
        assert!(!none.contains("Available project @mentions"));
    }

    #[test]
    fn parse_happy_path_parallel() {
        let out = r#"Sure, here is the split.
<<<TASKPLAN
{"difficulty": 6, "parallel": true, "units": [
  {"title": "a.mov", "prompt": "Transcode ./media/a.mov to webm", "mentions": ["media"]},
  {"title": "b.mov", "prompt": "Transcode ./media/b.mov to webm"}
], "rationale": "two independent files"}
TASKPLAN
"#;
        let plan = parse_task_plan(out).expect("plan");
        assert_eq!(plan.difficulty, 6);
        assert!(plan.parallel);
        assert_eq!(plan.units.len(), 2);
        assert_eq!(plan.units[0].title, "a.mov");
        assert_eq!(plan.units[0].mentions, vec!["media".to_string()]);
        assert!(plan.units[1].mentions.is_empty());
        assert_eq!(plan.rationale, "two independent files");
    }

    #[test]
    fn parse_tolerates_ansi_and_surrounding_noise() {
        // CSI colour codes wrapped around the block, plus trailing chatter.
        let out = "\u{1b}[32mdone\u{1b}[0m\n<<<TASKPLAN\n{\"difficulty\": 3, \"parallel\": \
                   false, \"units\": [{\"title\": \"t\", \"prompt\": \"do it\"}], \
                   \"rationale\": \"one cohesive change\"}\nTASKPLAN\n\u{1b}[2mbye\u{1b}[0m";
        let plan = parse_task_plan(out).expect("plan");
        assert_eq!(plan.difficulty, 3);
        assert!(!plan.parallel);
        assert_eq!(plan.units.len(), 1);
    }

    #[test]
    fn parse_missing_fence_is_none() {
        assert!(parse_task_plan("no plan here, just prose").is_none());
    }

    #[test]
    fn parse_malformed_json_is_none() {
        let out = "<<<TASKPLAN\n{not valid json,,,}\nTASKPLAN";
        assert!(parse_task_plan(out).is_none());
    }

    #[test]
    fn parse_non_parallel_collapses_to_single_unit() {
        // Model wrongly returns parallel:true with one unit — we force parallel off.
        let out = "<<<TASKPLAN\n{\"difficulty\": 4, \"parallel\": true, \"units\": \
                   [{\"title\": \"only\", \"prompt\": \"the whole task\"}], \"rationale\": \"x\"}\n\
                   TASKPLAN";
        let plan = parse_task_plan(out).expect("plan");
        assert!(!plan.parallel);
        assert_eq!(plan.units.len(), 1);
    }

    #[test]
    fn parse_clamps_difficulty_and_rounds() {
        let out = "<<<TASKPLAN\n{\"difficulty\": 42, \"parallel\": false, \"units\": \
                   [{\"prompt\": \"p\"}], \"rationale\": \"\"}\nTASKPLAN";
        assert_eq!(parse_task_plan(out).unwrap().difficulty, 10);
        let low = "<<<TASKPLAN\n{\"difficulty\": 0, \"parallel\": false, \"units\": \
                   [{\"prompt\": \"p\"}], \"rationale\": \"\"}\nTASKPLAN";
        assert_eq!(parse_task_plan(low).unwrap().difficulty, 1);
    }

    #[test]
    fn parse_drops_blank_units_and_falls_back_title() {
        let out = "<<<TASKPLAN\n{\"difficulty\": 5, \"parallel\": true, \"units\": [\
                   {\"title\": \"\", \"prompt\": \"first\"},\
                   {\"title\": \"x\", \"prompt\": \"   \"},\
                   {\"title\": \"y\", \"prompt\": \"second\"}\
                   ], \"rationale\": \"r\"}\nTASKPLAN";
        let plan = parse_task_plan(out).expect("plan");
        // Blank-prompt unit dropped; two remain, so parallel stays on.
        assert_eq!(plan.units.len(), 2);
        assert!(plan.parallel);
        // Missing title falls back to the prompt text.
        assert_eq!(plan.units[0].title, "first");
    }

    #[test]
    fn parse_truncates_to_max_units() {
        let mut items = String::new();
        for i in 0..(MAX_UNITS + 5) {
            if i > 0 {
                items.push(',');
            }
            items.push_str(&format!("{{\"title\": \"t{i}\", \"prompt\": \"p{i}\"}}"));
        }
        let out = format!(
            "<<<TASKPLAN\n{{\"difficulty\": 9, \"parallel\": true, \"units\": [{items}], \
             \"rationale\": \"many\"}}\nTASKPLAN"
        );
        let plan = parse_task_plan(&out).expect("plan");
        assert_eq!(plan.units.len(), MAX_UNITS);
    }

    #[test]
    fn parse_uses_last_fence() {
        // The model restated the format earlier; the real answer is the final block.
        let out = "example: <<<TASKPLAN\n{\"difficulty\": 1, \"parallel\": false, \"units\": \
                   [{\"prompt\": \"ignored\"}], \"rationale\": \"template\"}\nTASKPLAN\n\
                   Now the answer:\n<<<TASKPLAN\n{\"difficulty\": 8, \"parallel\": true, \
                   \"units\": [{\"prompt\": \"a\"}, {\"prompt\": \"b\"}], \"rationale\": \"real\"}\n\
                   TASKPLAN";
        let plan = parse_task_plan(out).expect("plan");
        assert_eq!(plan.difficulty, 8);
        assert_eq!(plan.rationale, "real");
    }
}
