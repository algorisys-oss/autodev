//! Declarative description of how to launch an agent backend CLI.
//!
//! A `BackendSpec` turns `AgentOptions` into a program + argument vector by data, not code.
//! One spec per backend CLI; adding a backend is adding a spec (a bundled default or, later,
//! a JSON file on disk) rather than editing `command_line`. The canonical `build_args` order
//! is fixed and reproduces every backend the app shipped with — the `agent.rs` tests are its
//! conformance suite.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::agent::AgentOptions;

/// How a backend consumes attached image (screenshot) paths.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum ImageMode {
    /// Emit `<flag> <path>` for each image (e.g. Codex `-i`).
    Flag { flag: String },
    /// Append `text` (with `{path}` substituted per image) to the prompt string, because the
    /// CLI has no image flag (Claude, `agy`).
    AppendToPrompt { text: String },
    /// Backend has no image support; images are ignored.
    None,
}

/// How the initial prompt is passed to the CLI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum PromptMode {
    /// Trailing positional argument (Claude, Codex).
    Positional,
    /// Passed via a flag, e.g. `agy -i <prompt>`.
    Flag { flag: String },
}

fn default_image_mode() -> ImageMode {
    ImageMode::None
}

fn default_prompt_mode() -> PromptMode {
    PromptMode::Positional
}

/// Declarative launch recipe for one backend CLI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendSpec {
    /// Stable identifier the UI selects a backend by (e.g. `"claude"`).
    pub id: String,
    /// Human-facing name shown in the backend picker. Defaults to the `id` when absent.
    #[serde(default)]
    pub label: Option<String>,
    /// The executable to launch (e.g. `"claude"`, `"agy"`).
    pub program: String,
    /// Model ids offered for this backend in the UI. Not used to build args.
    #[serde(default)]
    pub models: Vec<String>,
    /// Emitted in one-shot/print mode (Claude `-p`). Takes precedence over `plan_flag`.
    #[serde(default)]
    pub print_flag: Vec<String>,
    /// Emitted for plan mode when NOT in print mode (Claude `--permission-mode plan`).
    #[serde(default)]
    pub plan_flag: Vec<String>,
    /// Emitted when bypass/yolo permissions are on.
    #[serde(default)]
    pub bypass_flag: Vec<String>,
    /// Flag preceding the model value (Claude `--model`, Codex/`agy` `-m`). None = unsupported.
    #[serde(default)]
    pub model_flag: Option<String>,
    /// Flag preceding each extra directory (Claude/`agy` `--add-dir`). None = unsupported.
    #[serde(default)]
    pub add_dir_flag: Option<String>,
    /// Prepend the working directory as the first add-dir. `agy` writes into its workspace
    /// rather than its process cwd, so the cwd must be added explicitly or deliverables land
    /// in a scratch project.
    #[serde(default)]
    pub add_cwd_to_dirs: bool,
    /// How attached images are passed.
    #[serde(default = "default_image_mode")]
    pub images: ImageMode,
    /// How the prompt is passed.
    #[serde(default = "default_prompt_mode")]
    pub prompt: PromptMode,
}

impl BackendSpec {
    /// The name to show in the backend picker: the explicit `label`, else the `id`.
    pub fn display_label(&self) -> String {
        self.label.clone().unwrap_or_else(|| self.id.clone())
    }

    /// Build the argument vector for `opts` under this spec. Order is fixed:
    /// print|plan → bypass → model → add-dirs → image flags → prompt (+ appended images).
    pub fn build_args(&self, opts: &AgentOptions) -> Vec<String> {
        let mut args: Vec<String> = Vec::new();

        // Print (one-shot) takes precedence over plan mode.
        if opts.print_mode && !self.print_flag.is_empty() {
            args.extend(self.print_flag.iter().cloned());
        } else if opts.plan_mode && !self.plan_flag.is_empty() {
            args.extend(self.plan_flag.iter().cloned());
        }

        if opts.bypass_permissions && !self.bypass_flag.is_empty() {
            args.extend(self.bypass_flag.iter().cloned());
        }

        if let (Some(flag), Some(model)) = (&self.model_flag, &opts.model) {
            args.push(flag.clone());
            args.push(model.clone());
        }

        if let Some(flag) = &self.add_dir_flag {
            if self.add_cwd_to_dirs {
                args.push(flag.clone());
                args.push(opts.cwd.clone());
            }
            for dir in &opts.add_dirs {
                // Skip the cwd if it's already been prepended, so it isn't passed twice.
                if self.add_cwd_to_dirs && dir == &opts.cwd {
                    continue;
                }
                args.push(flag.clone());
                args.push(dir.clone());
            }
        }

        if let ImageMode::Flag { flag } = &self.images {
            for img in &opts.images {
                args.push(flag.clone());
                args.push(img.clone());
            }
        }

        // Build the prompt, appending image references first if that's how this backend
        // takes images, then emit it positionally or via a flag — but only if non-empty.
        let mut prompt = opts.initial_prompt.clone().unwrap_or_default();
        if let ImageMode::AppendToPrompt { text } = &self.images {
            for img in &opts.images {
                prompt.push_str(&text.replace("{path}", img));
            }
        }
        if !prompt.is_empty() {
            match &self.prompt {
                PromptMode::Positional => args.push(prompt),
                PromptMode::Flag { flag } => {
                    args.push(flag.clone());
                    args.push(prompt);
                }
            }
        }

        args
    }
}

/// The backends the app ships with. Verified against the installed CLIs (`agy` per Google's
/// published guide). Adding a backend here — or, from Step 2, dropping a JSON file — is the
/// only change needed to support a new CLI.
pub fn builtin_specs() -> Vec<BackendSpec> {
    vec![claude_spec(), codex_spec(), antigravity_spec()]
}

/// Every available backend: bundled defaults plus any `<data_dir>/backends/*.json`. A disk
/// spec whose `id` matches a bundled one replaces it, so users can both add new backends and
/// retune shipped ones. Malformed or unreadable files are skipped rather than failing the app.
/// Hermetic (`_from(dir)`); `load_specs` wraps it with the real data dir.
pub fn load_specs_from(data_dir: &Path) -> Vec<BackendSpec> {
    let mut specs = builtin_specs();
    let dir = data_dir.join("backends");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return specs; // no backends dir yet — bundled defaults only
    };
    let mut extra: Vec<BackendSpec> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("json"))
        .filter_map(|p| std::fs::read_to_string(&p).ok())
        .filter_map(|s| serde_json::from_str::<BackendSpec>(&s).ok())
        .collect();
    // Deterministic order regardless of directory iteration order.
    extra.sort_by(|a, b| a.id.cmp(&b.id));
    for spec in extra {
        match specs.iter().position(|s| s.id == spec.id) {
            Some(i) => specs[i] = spec,
            None => specs.push(spec),
        }
    }
    specs
}

/// Every available backend using the app's real data dir (`~/.autodev`). Falls back to
/// bundled defaults if the home dir can't be resolved.
pub fn load_specs() -> Vec<BackendSpec> {
    match crate::state::data_dir() {
        Ok(dir) => load_specs_from(&dir),
        Err(_) => builtin_specs(),
    }
}

fn screenshot_append() -> ImageMode {
    ImageMode::AppendToPrompt {
        text: "\n\n[Screenshot attached: {path}]".to_string(),
    }
}

fn claude_spec() -> BackendSpec {
    BackendSpec {
        id: "claude".into(),
        label: Some("Claude".into()),
        program: "claude".into(),
        models: vec![],
        print_flag: vec!["-p".into()],
        plan_flag: vec!["--permission-mode".into(), "plan".into()],
        bypass_flag: vec!["--dangerously-skip-permissions".into()],
        model_flag: Some("--model".into()),
        add_dir_flag: Some("--add-dir".into()),
        add_cwd_to_dirs: false,
        images: screenshot_append(),
        prompt: PromptMode::Positional,
    }
}

fn codex_spec() -> BackendSpec {
    BackendSpec {
        id: "codex".into(),
        label: Some("Codex".into()),
        program: "codex".into(),
        models: vec![],
        print_flag: vec![],
        plan_flag: vec![],
        bypass_flag: vec!["--dangerously-bypass-approvals-and-sandbox".into()],
        model_flag: Some("-m".into()),
        add_dir_flag: None,
        add_cwd_to_dirs: false,
        images: ImageMode::Flag { flag: "-i".into() },
        prompt: PromptMode::Positional,
    }
}

fn antigravity_spec() -> BackendSpec {
    BackendSpec {
        id: "antigravity".into(),
        label: Some("Antigravity".into()),
        program: "agy".into(),
        models: vec![],
        print_flag: vec![],
        // No documented plan/read-only flag for `agy`; plan mode is not mapped.
        plan_flag: vec![],
        bypass_flag: vec!["--dangerously-skip-permissions".into()],
        model_flag: Some("-m".into()),
        add_dir_flag: Some("--add-dir".into()),
        add_cwd_to_dirs: true,
        images: screenshot_append(),
        prompt: PromptMode::Flag { flag: "-i".into() },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentBackend;

    fn opts() -> AgentOptions {
        AgentOptions {
            backend: AgentBackend::Claude,
            cwd: "/tmp".into(),
            plan_mode: false,
            bypass_permissions: false,
            print_mode: false,
            model: None,
            initial_prompt: None,
            add_dirs: vec![],
            images: vec![],
            mock_command: None,
        }
    }

    #[test]
    fn builtin_specs_have_expected_ids_and_programs() {
        let specs = builtin_specs();
        assert_eq!(
            specs.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            ["claude", "codex", "antigravity"]
        );
        let find = |id: &str| specs.iter().find(|s| s.id == id);
        assert_eq!(find("antigravity").unwrap().program, "agy");
        assert!(find("nonesuch").is_none());
    }

    #[test]
    fn image_flag_mode_emits_flag_per_image_before_positional_prompt() {
        let spec = codex_spec();
        let o = AgentOptions {
            images: vec!["/a.png".into(), "/b.png".into()],
            initial_prompt: Some("go".into()),
            ..opts()
        };
        assert_eq!(spec.build_args(&o), ["-i", "/a.png", "-i", "/b.png", "go"]);
    }

    #[test]
    fn append_image_mode_folds_paths_into_the_prompt() {
        let spec = claude_spec();
        let o = AgentOptions {
            images: vec!["/a.png".into()],
            initial_prompt: Some("look".into()),
            ..opts()
        };
        let args = spec.build_args(&o);
        assert_eq!(args.len(), 1);
        assert!(args[0].contains("look"));
        assert!(args[0].contains("[Screenshot attached: /a.png]"));
    }

    #[test]
    fn empty_prompt_is_not_emitted() {
        assert!(claude_spec().build_args(&opts()).is_empty());
    }

    fn temp_data_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "autodev-spec-{tag}-{:?}",
            std::thread::current().id()
        ));
        std::fs::create_dir_all(dir.join("backends")).unwrap();
        dir
    }

    /// The Step 2 claim: dropping a JSON file into `<data_dir>/backends/` registers a new
    /// backend with zero code, and a disk file may override a bundled one by id.
    #[test]
    fn load_specs_registers_disk_backends_and_overrides_builtins() {
        let data = temp_data_dir("load");
        // A brand-new backend...
        std::fs::write(
            data.join("backends/opencode.json"),
            r#"{ "id": "opencode", "program": "opencode", "bypassFlag": ["--yolo"] }"#,
        )
        .unwrap();
        // ...and an override of a bundled one (different program).
        std::fs::write(
            data.join("backends/claude.json"),
            r#"{ "id": "claude", "program": "claude-next" }"#,
        )
        .unwrap();

        let specs = load_specs_from(&data);
        let by = |id: &str| specs.iter().find(|s| s.id == id).cloned();

        assert_eq!(by("opencode").unwrap().program, "opencode");
        assert_eq!(by("claude").unwrap().program, "claude-next");
        // Codex is untouched, and nothing is duplicated.
        assert_eq!(by("codex").unwrap().program, "codex");
        assert_eq!(specs.iter().filter(|s| s.id == "claude").count(), 1);

        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn load_specs_from_missing_dir_yields_builtins() {
        let missing = std::env::temp_dir().join("autodev-spec-does-not-exist-xyz");
        assert_eq!(load_specs_from(&missing).len(), builtin_specs().len());
    }

    /// The heart of P1: a spec that arrives as data (JSON) builds the same args as code.
    #[test]
    fn spec_deserialized_from_json_builds_args() {
        let json = r#"{
            "id": "demo",
            "program": "demo-cli",
            "bypassFlag": ["--yolo"],
            "modelFlag": "--model",
            "addDirFlag": "--dir",
            "images": { "mode": "flag", "flag": "-img" },
            "prompt": { "mode": "positional" }
        }"#;
        let spec: BackendSpec = serde_json::from_str(json).unwrap();
        assert_eq!(spec.program, "demo-cli");
        let o = AgentOptions {
            bypass_permissions: true,
            model: Some("m1".into()),
            add_dirs: vec!["/ctx".into()],
            images: vec!["/s.png".into()],
            initial_prompt: Some("hi".into()),
            ..opts()
        };
        assert_eq!(
            spec.build_args(&o),
            ["--yolo", "--model", "m1", "--dir", "/ctx", "-img", "/s.png", "hi"]
        );
    }
}
