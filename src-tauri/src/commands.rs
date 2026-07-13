use std::io::Write;
use std::sync::{Arc, Mutex};

use base64::Engine;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::agent::{AgentInfo, AgentManager, AgentOptions};
use crate::error::{AppError, AppResult};
use crate::state::{self, AppSettings};
use crate::workspace::{self, ResolvedMention, Workspace, WorkspaceStore};

/// Load the workspace store from the real data directory.
fn load_store() -> AppResult<WorkspaceStore> {
    workspace::load_store_from(&state::data_dir()?)
}

/// Save the workspace store to the real data directory.
fn save_store(store: &WorkspaceStore) -> AppResult<()> {
    workspace::save_store_to(&state::data_dir()?, store)
}

/// Static info about the running app. Used by the frontend on startup to prove
/// the command+event bridge is wired and to show the version.
#[derive(Debug, Serialize)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        name: "AutoDev".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

/// A backend the UI can launch, for the composer's picker. Sourced from the bundled
/// defaults plus any disk-registered specs (`~/.autodev/backends/*.json`).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendInfo {
    pub id: String,
    pub label: String,
    pub models: Vec<String>,
    /// Whether this backend can emit a structured event stream (offers the Rich view).
    pub structured: bool,
}

/// List every available backend so the frontend can build its picker without hardcoding
/// them. `mock` is excluded — it is a test-only backend, not a user-selectable one.
#[tauri::command]
pub fn backend_list() -> Vec<BackendInfo> {
    crate::backend_spec::load_specs()
        .into_iter()
        .map(|s| BackendInfo {
            label: s.display_label(),
            structured: s.structured.is_some(),
            id: s.id,
            models: s.models,
        })
        .collect()
}

/// Reusable prompt templates from `~/.autodev/templates/*.md`, for the composer's `/name`
/// expansion.
#[tauri::command]
pub fn list_templates() -> AppResult<Vec<crate::templates::PromptTemplate>> {
    crate::templates::list_templates()
}

/// The skills directory (`~/.autodev/skills`) if it exists and has content, so the frontend
/// can add it to every agent's context via a spawn hook. `None` otherwise.
#[tauri::command]
pub fn skills_dir() -> AppResult<Option<String>> {
    crate::templates::skills_dir()
}

/// Executable extensions (`~/.autodev/extensions/*.js`) as name + source. The frontend runs
/// each as a module against the `autodev` API. These run untrusted-user code with the app's
/// full privileges — surfaced, not sandboxed (they are the user's own files).
#[tauri::command]
pub fn list_extensions() -> AppResult<Vec<crate::extensions::ExtensionFile>> {
    crate::extensions::list_extensions()
}

#[tauri::command]
pub fn get_settings() -> AppResult<AppSettings> {
    state::load_settings()
}

#[tauri::command]
pub fn set_settings(settings: AppSettings) -> AppResult<AppSettings> {
    state::save_settings(&settings)?;
    Ok(settings)
}

/// Open `path` (an agent's worktree or cwd) in the configured editor. Spawns the editor
/// detached — GUI editors return immediately — and never blocks the app.
#[tauri::command]
pub fn open_in_editor(path: String) -> AppResult<()> {
    let editor = state::load_settings()?
        .editor_command
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("code")
        .to_string();
    let (program, args) = crate::editor::build_open_command(&editor, &path)?;
    std::process::Command::new(&program)
        .args(&args)
        .spawn()
        .map_err(AppError::Io)?;
    Ok(())
}

#[tauri::command]
pub fn list_workspaces() -> AppResult<Vec<Workspace>> {
    Ok(load_store()?.workspaces)
}

#[tauri::command]
pub fn create_workspace(name: String) -> AppResult<Workspace> {
    let mut store = load_store()?;
    let ws = store.create_workspace(&name);
    save_store(&store)?;
    Ok(ws)
}

#[tauri::command]
pub fn delete_workspace(id: String) -> AppResult<()> {
    let mut store = load_store()?;
    store.delete_workspace(&id)?;
    save_store(&store)
}

#[tauri::command]
pub fn add_project(workspace_id: String, path: String) -> AppResult<Workspace> {
    let mut store = load_store()?;
    let ws = store.add_project(&workspace_id, &path)?;
    save_store(&store)?;
    Ok(ws)
}

#[tauri::command]
pub fn remove_project(workspace_id: String, project_name: String) -> AppResult<Workspace> {
    let mut store = load_store()?;
    let ws = store.remove_project(&workspace_id, &project_name)?;
    save_store(&store)?;
    Ok(ws)
}

#[tauri::command]
pub fn resolve_mention(workspace_id: String, token: String) -> AppResult<Option<ResolvedMention>> {
    let store = load_store()?;
    let ws = store
        .workspaces
        .iter()
        .find(|w| w.id == workspace_id)
        .ok_or_else(|| crate::error::AppError::NotFound(format!("workspace {workspace_id}")))?;
    Ok(workspace::resolve_mention(ws, &token))
}

// --- Agents (Phase 2) ---

#[derive(Clone, Serialize)]
struct OutputEvent {
    id: String,
    /// base64 of the raw PTY bytes, so terminal escape sequences survive intact.
    data: String,
}

#[derive(Clone, Serialize)]
struct ExitEvent {
    id: String,
    code: Option<i32>,
}

/// One normalized event from a Rich (structured) session, carried on `agent://event`.
#[derive(Clone, Serialize)]
struct AgentEventPayload {
    id: String,
    event: crate::agent_event::AgentEvent,
}

/// If `opts` requests Rich mode and its backend declares a `structured` driver, build that
/// driver; otherwise `None` (the session streams only raw terminal bytes).
fn rich_driver_for(opts: &AgentOptions) -> Option<Box<dyn crate::agent_event::StructuredDriver>> {
    if !opts.rich {
        return None;
    }
    let id = opts.backend.id();
    let specs = crate::backend_spec::load_specs();
    let name = specs
        .iter()
        .find(|s| s.id == id)
        .and_then(|s| s.structured.as_ref())
        .map(|m| m.driver.clone())?;
    crate::agent_event::driver_for(&name)
}

#[tauri::command]
pub fn agent_spawn(
    app: AppHandle,
    manager: State<'_, AgentManager>,
    options: AgentOptions,
    cols: Option<u16>,
    rows: Option<u16>,
) -> AppResult<String> {
    let id = manager.next_id();
    let cols = cols.unwrap_or(80);
    let rows = rows.unwrap_or(24);

    // Append raw output to a per-agent log on disk, so scrollback survives a crash
    // and the run is auditable (LOOPS XXX). Best-effort: logging never blocks a spawn.
    let log: Option<Arc<Mutex<std::fs::File>>> = state::logs_dir().ok().and_then(|dir| {
        std::fs::create_dir_all(&dir).ok()?;
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join(format!("{id}.log")))
            .ok()
            .map(|f| Arc::new(Mutex::new(f)))
    });

    // In Rich mode the raw stdout is a structured event stream; a driver parses it into
    // normalized `agent://event`s. The raw bytes are still emitted + logged (so the disk log
    // and a raw/debug terminal view keep working). `None` for a plain terminal session.
    let driver: Option<Arc<Mutex<Box<dyn crate::agent_event::StructuredDriver>>>> =
        rich_driver_for(&options).map(|d| Arc::new(Mutex::new(d)));

    let out_app = app.clone();
    let out_id = id.clone();
    let on_output = move |bytes: Vec<u8>| {
        if let Some(log) = &log {
            if let Ok(mut f) = log.lock() {
                let _ = f.write_all(&bytes);
            }
        }
        if let Some(driver) = &driver {
            let events = driver
                .lock()
                .map(|mut d| d.feed(&bytes))
                .unwrap_or_default();
            for event in events {
                let _ = out_app.emit(
                    "agent://event",
                    AgentEventPayload {
                        id: out_id.clone(),
                        event,
                    },
                );
            }
        }
        let data = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let _ = out_app.emit(
            "agent://output",
            OutputEvent {
                id: out_id.clone(),
                data,
            },
        );
    };

    let exit_app = app.clone();
    let exit_id = id.clone();
    let on_exit = move |code: Option<i32>| {
        let _ = exit_app.emit(
            "agent://exit",
            ExitEvent {
                id: exit_id.clone(),
                code,
            },
        );
    };

    let session =
        crate::agent::spawn_session(id.clone(), &options, cols, rows, on_output, on_exit)?;
    manager.insert(session);
    Ok(id)
}

#[tauri::command]
pub fn agent_write(manager: State<'_, AgentManager>, id: String, data: String) -> AppResult<()> {
    manager.get(&id)?.write(data.as_bytes())
}

#[tauri::command]
pub fn agent_resize(
    manager: State<'_, AgentManager>,
    id: String,
    cols: u16,
    rows: u16,
) -> AppResult<()> {
    manager.get(&id)?.resize(cols, rows)
}

#[tauri::command]
pub fn agent_kill(manager: State<'_, AgentManager>, id: String) -> AppResult<()> {
    let session = manager.get(&id)?;
    session.kill()?;
    Ok(())
}

#[tauri::command]
pub fn agent_list(manager: State<'_, AgentManager>) -> Vec<AgentInfo> {
    manager.list()
}

#[tauri::command]
pub fn agent_kill_all(manager: State<'_, AgentManager>) -> usize {
    manager.kill_all()
}

// --- Prompt history (Phase 4) ---

#[tauri::command]
pub fn get_prompt_history() -> AppResult<Vec<String>> {
    state::load_prompts()
}

#[tauri::command]
pub fn add_prompt_history(text: String) -> AppResult<Vec<String>> {
    state::add_prompt(&text)
}

// --- Git worktrees (Phase 5) ---

use crate::git::{self, WorktreeStatus};
use std::path::{Path, PathBuf};

/// A worktree created for isolated agent work.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeInfo {
    pub repo: String,
    pub path: String,
    pub branch: String,
}

#[tauri::command]
pub fn git_is_repo(dir: String) -> bool {
    git::is_repo(Path::new(&dir))
}

/// Turn a branch name into a directory-safe slug for the worktree folder.
fn worktree_slug(branch: &str) -> String {
    branch
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect()
}

#[tauri::command]
pub fn git_create_worktree(repo: String, branch: String) -> AppResult<WorktreeInfo> {
    let repo_path = PathBuf::from(&repo);
    let path = state::data_dir()?
        .join("worktrees")
        .join(worktree_slug(&branch));
    if path.exists() {
        return Err(crate::error::AppError::Conflict(format!(
            "worktree path already exists: {}",
            path.display()
        )));
    }
    std::fs::create_dir_all(path.parent().unwrap())?;
    git::create_worktree(&repo_path, &path, &branch)?;
    Ok(WorktreeInfo {
        repo,
        path: path.to_string_lossy().to_string(),
        branch,
    })
}

#[tauri::command]
pub fn git_worktree_status(path: String) -> AppResult<WorktreeStatus> {
    git::status(Path::new(&path))
}

#[tauri::command]
pub fn git_diff(repo: String, branch: String) -> AppResult<String> {
    git::diff(Path::new(&repo), &branch)
}

#[tauri::command]
pub fn git_merge_worktree(repo: String, branch: String) -> AppResult<String> {
    git::merge(Path::new(&repo), &branch)
}

#[tauri::command]
pub fn git_remove_worktree(repo: String, path: String, force: bool) -> AppResult<()> {
    git::remove_worktree(Path::new(&repo), Path::new(&path), force)
}

// --- Voice-to-text (Phase 6) ---

/// Write recorded audio bytes to a temp file, run the configured transcription command,
/// and return the transcript. Errors clearly if no `transcribeCommand` is set.
#[tauri::command]
pub fn transcribe_audio(data: Vec<u8>, ext: String) -> AppResult<String> {
    let template = state::load_settings()?.transcribe_command.ok_or_else(|| {
        crate::error::AppError::Transcribe(
            "no transcribeCommand configured in settings (~/.autodev/settings.json)".into(),
        )
    })?;
    let dir = state::data_dir()?.join("tmp");
    std::fs::create_dir_all(&dir)?;
    let safe_ext: String = ext.chars().filter(|c| c.is_alphanumeric()).collect();
    let file = dir.join(format!(
        "recording.{}",
        if safe_ext.is_empty() {
            "webm".into()
        } else {
            safe_ext
        }
    ));
    std::fs::write(&file, &data)?;
    let result = crate::transcribe::run_transcription(&template, &file);
    let _ = std::fs::remove_file(&file);
    result
}

// --- Screenshot + annotate (Phase 7) ---

/// Capture the screen via the configured `screenshotCommand` and return a base64 PNG.
#[tauri::command]
pub fn capture_screen() -> AppResult<String> {
    // Prefer an explicitly configured command; otherwise fall back to a detected platform
    // tool so screenshots work out of the box.
    let template = match state::load_settings()?
        .screenshot_command
        .filter(|t| !t.trim().is_empty())
    {
        Some(t) => t,
        None => crate::capture::default_screenshot_template().ok_or_else(|| {
            crate::error::AppError::Capture(
                "No screenshot tool found. Install one (e.g. grim, scrot, spectacle, or \
                 gnome-screenshot on Linux) or set a Screenshot command in Settings."
                    .into(),
            )
        })?,
    };
    let dir = state::data_dir()?.join("tmp");
    std::fs::create_dir_all(&dir)?;
    let file = dir.join("capture.png");
    let bytes = crate::capture::run_capture(&template, &file)?;
    let _ = std::fs::remove_file(&file);
    Ok(base64::engine::general_purpose::STANDARD.encode(&bytes))
}

/// Save an annotated PNG (base64) under `~/.autodev/shots/` and return its path.
#[tauri::command]
pub fn save_shot(data: String) -> AppResult<String> {
    let dir = state::data_dir()?.join("shots");
    let path = crate::capture::save_png(&dir, &data)?;
    Ok(path.to_string_lossy().to_string())
}

// --- Browser handoff (Phase 8) ---

#[tauri::command]
pub fn generate_handoff(task: String, url: String, context: String) -> String {
    crate::handoff::build_handoff(&task, &url, &context)
}

/// Run the configured `browserCommand` against the handoff text. Errors clearly if no
/// command is set (the handoff is still useful to copy into a browser AI manually).
#[tauri::command]
pub fn run_browser_handoff(handoff: String) -> AppResult<String> {
    let template = state::load_settings()?.browser_command.ok_or_else(|| {
        crate::error::AppError::Browser(
            "no browserCommand configured in settings; copy the handoff into a browser AI instead"
                .into(),
        )
    })?;
    let dir = state::data_dir()?.join("tmp");
    std::fs::create_dir_all(&dir)?;
    let file = dir.join("handoff.txt");
    std::fs::write(&file, &handoff)?;
    let result = crate::handoff::run_browser(&template, &file);
    let _ = std::fs::remove_file(&file);
    result
}

// --- Autonomous loop engine (Phase 9) ---

use crate::loop_engine::{self, LoopState, Role};

fn loop_slug(spec: &str) -> String {
    let base: String = spec
        .trim()
        .to_lowercase()
        .chars()
        .take(30)
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("{}-{nonce}", base.trim_matches('-'))
}

#[tauri::command]
pub fn loop_create(
    spec: String,
    project_dir: String,
    verify_command: Option<String>,
    max_iterations: Option<u32>,
    continue_on_failure: Option<bool>,
) -> AppResult<LoopState> {
    let id = loop_slug(&spec);
    let verify = verify_command.filter(|c| !c.trim().is_empty());
    let cap = max_iterations.unwrap_or(loop_engine::DEFAULT_MAX_ITERATIONS);
    let mut state = LoopState::with_options(id.clone(), spec, project_dir, verify, cap);
    state.continue_on_failure = continue_on_failure.unwrap_or(false);
    let base = state::loops_dir()?;
    loop_engine::save(&base, &state)?;
    loop_engine::append_log(&base, &id, "created; phase=planning")?;
    Ok(state)
}

#[tauri::command]
pub fn loop_get(id: String) -> AppResult<LoopState> {
    loop_engine::load(&state::loops_dir()?, &id)
}

#[tauri::command]
pub fn loop_list() -> AppResult<Vec<LoopState>> {
    let base = state::loops_dir()?;
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&base) {
        for e in entries.flatten() {
            if let Some(name) = e.file_name().to_str() {
                if let Ok(s) = loop_engine::load(&base, name) {
                    out.push(s);
                }
            }
        }
    }
    Ok(out)
}

/// HEAD commit of the loop's project dir, if it is a git repo — the base a generation round
/// is diffed against. `None` (no diff) when the dir is not a repo.
fn capture_base(project_dir: &str) -> Option<String> {
    let p = Path::new(project_dir);
    if git::is_repo(p) {
        git::head_commit(p).ok()
    } else {
        None
    }
}

/// The work-tree diff since the round's base commit — what the evaluator should grade.
fn round_diff(s: &LoopState) -> String {
    match &s.base_commit {
        Some(base) => git::diff_since(Path::new(&s.project_dir), base).unwrap_or_default(),
        None => String::new(),
    }
}

/// Grade one round: run the ground-truth verify command (if configured), apply the verdicts + test
/// result, record a bounded progress-memory line, re-base a retry's diff, and persist. Shared by
/// the manual (`loop_grade`) and auto (`loop_apply_evaluator`) paths so they behave identically.
fn apply_grade(
    base: &Path,
    mut s: LoopState,
    verdicts: &[bool],
    how: &str,
) -> AppResult<LoopState> {
    let round = s.iteration + 1;
    let verify = s
        .verify_command
        .as_deref()
        .map(|cmd| crate::verify::run_verify(cmd, Path::new(&s.project_dir)));
    let verify_pass = verify.as_ref().map(|o| o.passed);

    loop_engine::grade_and_advance(&mut s, verdicts, verify_pass);

    let vnote = match verify_pass {
        Some(true) => "verify=pass",
        Some(false) => "verify=fail",
        None => "verify=n/a",
    };
    let failing: Vec<&str> = s
        .contract
        .iter()
        .filter(|c| c.met != Some(true))
        .map(|c| c.text.as_str())
        .collect();
    let mut line = format!(
        "round {round}: {}/{} met; {vnote}",
        s.met_count(),
        s.contract.len()
    );
    if !failing.is_empty() {
        line.push_str(&format!("; failing: {}", failing.join(", ")));
    }
    loop_engine::append_progress(&mut s.progress, &line, 15);

    // A retry starts a fresh generation round; re-base the diff on the current HEAD.
    if s.phase == loop_engine::LoopPhase::Generating {
        s.base_commit = capture_base(&s.project_dir);
    }
    loop_engine::save(base, &s)?;
    loop_engine::append_log(
        base,
        &s.id,
        &format!(
            "{how}: phase={:?} iteration={} {vnote}{}",
            s.phase,
            s.iteration,
            s.failure_reason
                .as_deref()
                .map(|r| format!(" — {r}"))
                .unwrap_or_default()
        ),
    )?;
    Ok(s)
}

/// Record the current feature's contract and move to Generating.
#[tauri::command]
pub fn loop_set_contract(id: String, criteria: Vec<String>) -> AppResult<LoopState> {
    let base = state::loops_dir()?;
    let mut s = loop_engine::load(&base, &id)?;
    s.contract = criteria
        .into_iter()
        .map(|text| loop_engine::Criterion { text, met: None })
        .collect();
    s.phase = loop_engine::LoopPhase::Generating;
    s.base_commit = capture_base(&s.project_dir);
    loop_engine::save(&base, &s)?;
    loop_engine::append_log(
        &base,
        &id,
        &format!("contract set: {} criteria", s.contract.len()),
    )?;
    Ok(s)
}

/// Record the epic's feature backlog and move to planning the first feature.
#[tauri::command]
pub fn loop_set_features(id: String, titles: Vec<String>) -> AppResult<LoopState> {
    let base = state::loops_dir()?;
    let mut s = loop_engine::load(&base, &id)?;
    let n = titles.len();
    loop_engine::set_features(&mut s, titles);
    loop_engine::save(&base, &s)?;
    loop_engine::append_log(&base, &id, &format!("backlog set: {n} features"))?;
    Ok(s)
}

/// Move Generating → Evaluating (the generator finished a round).
#[tauri::command]
pub fn loop_ready_to_evaluate(id: String) -> AppResult<LoopState> {
    let base = state::loops_dir()?;
    let mut s = loop_engine::load(&base, &id)?;
    s.phase = loop_engine::LoopPhase::Evaluating;
    loop_engine::save(&base, &s)?;
    loop_engine::append_log(&base, &id, "generation done; evaluating")?;
    Ok(s)
}

/// Apply the evaluator's per-criterion verdicts and advance (pass / retry / fail).
#[tauri::command]
pub fn loop_grade(id: String, verdicts: Vec<bool>) -> AppResult<LoopState> {
    let base = state::loops_dir()?;
    let s = loop_engine::load(&base, &id)?;
    apply_grade(&base, s, &verdicts, "graded")
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RolePrompt {
    pub role: Role,
    pub prompt: String,
}

/// The role + prompt to run for the loop's current phase (None once passed/failed). When the
/// evaluator is due, the diff of this generation round (work tree vs. the round's base commit)
/// is computed here and embedded in the prompt; a caller-supplied `diff` overrides it.
#[tauri::command]
pub fn loop_current_prompt(id: String, diff: String) -> AppResult<Option<RolePrompt>> {
    let s = loop_engine::load(&state::loops_dir()?, &id)?;
    let diff = if s.phase == loop_engine::LoopPhase::Evaluating && diff.trim().is_empty() {
        round_diff(&s)
    } else {
        diff
    };
    Ok(loop_engine::prompt_for_phase(&s, &diff).map(|(role, prompt)| RolePrompt { role, prompt }))
}

/// Whether the loop's progress memory has grown large enough to warrant a summarizer pass.
#[tauri::command]
pub fn loop_needs_compaction(id: String) -> AppResult<bool> {
    let s = loop_engine::load(&state::loops_dir()?, &id)?;
    Ok(loop_engine::needs_compaction(&s.progress))
}

/// The Summarizer role + prompt for compacting the loop's progress memory (a maintenance step,
/// not a phase). Runs read-only; the caller applies the result with `loop_compact`.
#[tauri::command]
pub fn loop_compact_prompt(id: String) -> AppResult<RolePrompt> {
    let s = loop_engine::load(&state::loops_dir()?, &id)?;
    Ok(RolePrompt {
        role: loop_engine::Role::Summarizer,
        prompt: loop_engine::summarizer_prompt(&s.spec, &s.backlog_overview(), &s.progress),
    })
}

/// Replace the loop's progress memory with the summarizer agent's compacted summary.
#[tauri::command]
pub fn loop_compact(id: String, agent_id: String) -> AppResult<LoopState> {
    let base = state::loops_dir()?;
    let mut s = loop_engine::load(&base, &id)?;
    let summary = loop_engine::parse_summary(&read_agent_log(&agent_id)?);
    if summary.trim().is_empty() {
        return Err(AppError::Conflict(
            "no summary found in the summarizer output".into(),
        ));
    }
    loop_engine::compact_progress(&mut s, &summary);
    loop_engine::save(&base, &s)?;
    loop_engine::append_log(&base, &id, "progress compacted by summarizer")?;
    Ok(s)
}

/// Read a finished agent's captured output log (lossy UTF-8; the log holds raw PTY bytes).
fn read_agent_log(agent_id: &str) -> AppResult<String> {
    let path = state::logs_dir()?.join(format!("{agent_id}.log"));
    let bytes = std::fs::read(&path)
        .map_err(|_| AppError::NotFound(format!("output log for agent {agent_id}")))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Auto-advance the decomposing phase: parse the decomposer agent's output into the feature
/// backlog and move to planning the first feature. Errors (leaving the loop in Decomposing) if
/// no features can be parsed, so the UI can fall back to manual entry.
#[tauri::command]
pub fn loop_apply_decomposer(id: String, agent_id: String) -> AppResult<LoopState> {
    let base = state::loops_dir()?;
    let mut s = loop_engine::load(&base, &id)?;
    if s.phase != loop_engine::LoopPhase::Decomposing {
        return Err(AppError::Conflict(format!(
            "loop {id} is not decomposing (phase is {:?})",
            s.phase
        )));
    }
    let titles = loop_engine::parse_features(&read_agent_log(&agent_id)?);
    if titles.is_empty() {
        return Err(AppError::Conflict(
            "no features found in the decomposer output; enter them manually".into(),
        ));
    }
    let n = titles.len();
    loop_engine::set_features(&mut s, titles);
    loop_engine::save(&base, &s)?;
    loop_engine::append_log(
        &base,
        &id,
        &format!("decomposer auto-applied: {n} features"),
    )?;
    Ok(s)
}

/// Auto-advance the planning phase: parse the planner agent's output into a contract and move
/// to Generating. Errors (leaving the loop in Planning) if no criteria can be parsed, so the
/// UI can fall back to manual entry.
#[tauri::command]
pub fn loop_apply_planner(id: String, agent_id: String) -> AppResult<LoopState> {
    let base = state::loops_dir()?;
    let mut s = loop_engine::load(&base, &id)?;
    if s.phase != loop_engine::LoopPhase::Planning {
        return Err(AppError::Conflict(format!(
            "loop {id} is not planning (phase is {:?})",
            s.phase
        )));
    }
    let criteria = loop_engine::parse_contract(&read_agent_log(&agent_id)?);
    if criteria.is_empty() {
        return Err(AppError::Conflict(
            "no contract criteria found in the planner output; enter them manually".into(),
        ));
    }
    let n = criteria.len();
    s.contract = criteria
        .into_iter()
        .map(|text| loop_engine::Criterion { text, met: None })
        .collect();
    s.phase = loop_engine::LoopPhase::Generating;
    s.base_commit = capture_base(&s.project_dir);
    loop_engine::save(&base, &s)?;
    loop_engine::append_log(&base, &id, &format!("planner auto-applied: {n} criteria"))?;
    Ok(s)
}

/// Auto-advance the evaluating phase: parse the evaluator agent's PASS/FAIL verdicts and grade
/// (pass / retry / fail). Errors (leaving the loop in Evaluating) if the log is missing, so the
/// UI can fall back to manual checkboxes.
#[tauri::command]
pub fn loop_apply_evaluator(id: String, agent_id: String) -> AppResult<LoopState> {
    let base = state::loops_dir()?;
    let s = loop_engine::load(&base, &id)?;
    if s.phase != loop_engine::LoopPhase::Evaluating {
        return Err(AppError::Conflict(format!(
            "loop {id} is not evaluating (phase is {:?})",
            s.phase
        )));
    }
    let verdicts = loop_engine::parse_verdicts(&read_agent_log(&agent_id)?, &s.contract);
    apply_grade(&base, s, &verdicts, "evaluator auto-graded")
}

// --- Task splitter (Phase 10): pre-launch parallel-decomposition classifier ---

/// Build the classifier prompt for `task`. Returned to the frontend so the exact wording
/// lives once in Rust; the frontend spawns a one-shot agent with it, then calls
/// `task_split_parse` on the agent's output.
#[tauri::command]
pub fn task_split_prompt(task: String, projects: Vec<String>) -> String {
    crate::task_split::split_prompt(&task, &projects)
}

/// Parse a finished classifier agent's output into a `TaskPlan`. Returns `None` (not an
/// error) when the output has no valid plan block, so the UI can fall back to manual setup.
#[tauri::command]
pub fn task_split_parse(agent_id: String) -> AppResult<Option<crate::task_split::TaskPlan>> {
    Ok(crate::task_split::parse_task_plan(&read_agent_log(
        &agent_id,
    )?))
}
