use std::path::Path;
use std::process::Command;

/// The outcome of running a loop's ground-truth verify command: whether it passed (exit 0) and a
/// bounded tail of its combined output, for the loop's progress log and the generator's next hint.
#[derive(Debug, Clone, PartialEq)]
pub struct VerifyOutcome {
    pub passed: bool,
    pub output: String,
}

/// Longest slice of verify output we keep — enough for a failing test's message, small enough not
/// to bloat the loop's progress memory.
const MAX_OUTPUT: usize = 2000;

/// Run the loop's verify command in `project_dir` via `sh -c`, so it can be any pipeline
/// (`./dev.sh verify`, `npm test && npm run lint`, …). Exit status 0 means the build's own tests
/// pass — the ground truth that overrides the evaluator agent's say-so. The command is
/// user-configured (not agent output) and run without interpolation.
pub fn run_verify(command: &str, project_dir: &Path) -> VerifyOutcome {
    let result = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(project_dir)
        .output();
    match result {
        Ok(out) => {
            let mut combined = String::from_utf8_lossy(&out.stdout).into_owned();
            combined.push_str(&String::from_utf8_lossy(&out.stderr));
            VerifyOutcome {
                passed: out.status.success(),
                output: tail(&combined, MAX_OUTPUT),
            }
        }
        // A command that cannot even be launched counts as a failed verification, not a pass.
        Err(e) => VerifyOutcome {
            passed: false,
            output: format!("verify command could not run: {e}"),
        },
    }
}

/// Keep the last `max` chars of `s` (verify failures put the useful message at the end).
fn tail(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        return t.to_string();
    }
    let start = t.chars().count() - max;
    t.chars().skip(start).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passing_command_reports_pass_and_output() {
        let out = run_verify("printf 'all tests passed'", &std::env::temp_dir());
        assert!(out.passed);
        assert_eq!(out.output, "all tests passed");
    }

    #[test]
    fn nonzero_exit_is_a_failure() {
        let out = run_verify("echo boom >&2; exit 1", &std::env::temp_dir());
        assert!(!out.passed);
        assert!(out.output.contains("boom"));
    }

    #[test]
    fn runs_in_the_project_dir() {
        let dir = std::env::temp_dir();
        let out = run_verify("test \"$(pwd)\" = \"$(cd; pwd)\" && echo home || pwd", &dir);
        // `pwd` should reflect project_dir, not $HOME.
        assert!(out.passed);
        assert!(out
            .output
            .contains(&dir.to_string_lossy().trim_end_matches('/').to_string()));
    }

    #[test]
    fn output_is_bounded() {
        let out = run_verify(
            "for i in $(seq 1 5000); do echo line$i; done",
            &std::env::temp_dir(),
        );
        assert!(out.passed);
        assert!(out.output.chars().count() <= MAX_OUTPUT);
    }
}
