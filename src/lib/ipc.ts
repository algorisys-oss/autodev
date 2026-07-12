// Typed wrappers over Tauri commands. This is the single place the frontend and
// Rust core agree on the command contract. Keep these types in sync with the
// structs in src-tauri/src/commands.rs and src-tauri/src/state.rs.
import { invoke } from "@tauri-apps/api/core";

export interface AppInfo {
  name: string;
  version: string;
}

export interface AppSettings {
  theme: "system" | "light" | "dark";
  defaultEffort: "high" | "extra-high";
  transcribeCommand?: string | null;
  screenshotCommand?: string | null;
  browserCommand?: string | null;
  editorCommand?: string | null;
  autoSplitOnLaunch?: boolean;
}

export function appInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info");
}

export function getSettings(): Promise<AppSettings> {
  return invoke<AppSettings>("get_settings");
}

/** Open a directory (an agent's worktree or cwd) in the configured editor. */
export function openInEditor(path: string): Promise<void> {
  return invoke<void>("open_in_editor", { path });
}

export function setSettings(settings: AppSettings): Promise<AppSettings> {
  return invoke<AppSettings>("set_settings", { settings });
}

// --- Workspaces & projects (Phase 1) ---

export interface Project {
  name: string;
  path: string;
}

export interface Workspace {
  id: string;
  name: string;
  projects: Project[];
}

export interface ResolvedMention {
  project: Project;
  files: string[];
  truncated: boolean;
}

export function listWorkspaces(): Promise<Workspace[]> {
  return invoke<Workspace[]>("list_workspaces");
}

export function createWorkspace(name: string): Promise<Workspace> {
  return invoke<Workspace>("create_workspace", { name });
}

export function deleteWorkspace(id: string): Promise<void> {
  return invoke<void>("delete_workspace", { id });
}

export function addProject(workspaceId: string, path: string): Promise<Workspace> {
  return invoke<Workspace>("add_project", { workspaceId, path });
}

export function removeProject(workspaceId: string, projectName: string): Promise<Workspace> {
  return invoke<Workspace>("remove_project", { workspaceId, projectName });
}

export function resolveMention(
  workspaceId: string,
  token: string,
): Promise<ResolvedMention | null> {
  return invoke<ResolvedMention | null>("resolve_mention", { workspaceId, token });
}

// --- Agents (Phase 2) ---

export type AgentBackend = "claude" | "codex" | "antigravity" | "mock";

export interface AgentOptions {
  backend: AgentBackend;
  cwd: string;
  planMode?: boolean;
  bypassPermissions?: boolean;
  /** One-shot mode (`claude -p`): run once, print, and exit. The loop needs this — auto-advance
   *  fires on agent exit, and interactive agents never exit. */
  printMode?: boolean;
  model?: string | null;
  initialPrompt?: string | null;
  addDirs?: string[];
  images?: string[];
  mockCommand?: string[] | null;
}

export interface AgentInfo {
  id: string;
  backend: AgentBackend;
  cwd: string;
  running: boolean;
}

export function agentSpawn(options: AgentOptions, cols?: number, rows?: number): Promise<string> {
  return invoke<string>("agent_spawn", { options, cols, rows });
}

export function agentWrite(id: string, data: string): Promise<void> {
  return invoke<void>("agent_write", { id, data });
}

export function agentResize(id: string, cols: number, rows: number): Promise<void> {
  return invoke<void>("agent_resize", { id, cols, rows });
}

export function agentKill(id: string): Promise<void> {
  return invoke<void>("agent_kill", { id });
}

export function agentList(): Promise<AgentInfo[]> {
  return invoke<AgentInfo[]>("agent_list");
}

export function agentKillAll(): Promise<number> {
  return invoke<number>("agent_kill_all");
}

// --- Prompt history (Phase 4) ---

export function getPromptHistory(): Promise<string[]> {
  return invoke<string[]>("get_prompt_history");
}

export function addPromptHistory(text: string): Promise<string[]> {
  return invoke<string[]>("add_prompt_history", { text });
}

// --- Git worktrees (Phase 5) ---

export interface WorktreeInfo {
  repo: string;
  path: string;
  branch: string;
}

export interface WorktreeStatus {
  branch: string;
  dirty: boolean;
}

export function gitIsRepo(dir: string): Promise<boolean> {
  return invoke<boolean>("git_is_repo", { dir });
}

export function gitCreateWorktree(repo: string, branch: string): Promise<WorktreeInfo> {
  return invoke<WorktreeInfo>("git_create_worktree", { repo, branch });
}

export function gitWorktreeStatus(path: string): Promise<WorktreeStatus> {
  return invoke<WorktreeStatus>("git_worktree_status", { path });
}

export function gitDiff(repo: string, branch: string): Promise<string> {
  return invoke<string>("git_diff", { repo, branch });
}

export function gitMergeWorktree(repo: string, branch: string): Promise<string> {
  return invoke<string>("git_merge_worktree", { repo, branch });
}

export function gitRemoveWorktree(repo: string, path: string, force: boolean): Promise<void> {
  return invoke<void>("git_remove_worktree", { repo, path, force });
}

// --- Voice-to-text (Phase 6) ---

export function transcribeAudio(data: Uint8Array, ext: string): Promise<string> {
  return invoke<string>("transcribe_audio", { data: Array.from(data), ext });
}

// --- Screenshot + annotate (Phase 7) ---

/** Capture the screen; returns a base64 PNG. */
export function captureScreen(): Promise<string> {
  return invoke<string>("capture_screen");
}

/** Save an annotated base64 PNG; returns its file path. */
export function saveShot(data: string): Promise<string> {
  return invoke<string>("save_shot", { data });
}

// --- Browser handoff (Phase 8) ---

export function generateHandoff(task: string, url: string, context: string): Promise<string> {
  return invoke<string>("generate_handoff", { task, url, context });
}

export function runBrowserHandoff(handoff: string): Promise<string> {
  return invoke<string>("run_browser_handoff", { handoff });
}

// --- Autonomous loop engine (Phase 9) ---

export type LoopPhase =
  | "decomposing"
  | "planning"
  | "generating"
  | "evaluating"
  | "passed"
  | "failed";
export type Role = "decomposer" | "planner" | "generator" | "evaluator" | "summarizer";

/** Progress-memory size past which the summarizer should compact it. Mirrors the Rust
 *  `loop_engine::MAX_PROGRESS_CHARS`. */
export const MAX_PROGRESS_CHARS = 2500;

/** Has a loop's progress memory grown large enough to warrant a compaction pass? */
export function needsCompaction(progress: string | undefined | null): boolean {
  return (progress?.length ?? 0) > MAX_PROGRESS_CHARS;
}

export interface Criterion {
  text: string;
  met: boolean | null;
}

export interface Feature {
  title: string;
  done: boolean;
  failed?: boolean;
}

export interface LoopState {
  id: string;
  spec: string;
  projectDir: string;
  phase: LoopPhase;
  iteration: number;
  maxIterations: number;
  contract: Criterion[];
  features: Feature[];
  currentFeature?: number;
  progress: string;
  baseCommit?: string | null;
  verifyCommand?: string | null;
  history?: number[];
  failureReason?: string | null;
  continueOnFailure?: boolean;
}

export interface RolePrompt {
  role: Role;
  prompt: string;
}

export function loopCreate(
  spec: string,
  projectDir: string,
  verifyCommand?: string | null,
  maxIterations?: number | null,
  continueOnFailure?: boolean,
): Promise<LoopState> {
  return invoke<LoopState>("loop_create", {
    spec,
    projectDir,
    verifyCommand: verifyCommand ?? null,
    maxIterations: maxIterations ?? null,
    continueOnFailure: continueOnFailure ?? false,
  });
}

export function loopList(): Promise<LoopState[]> {
  return invoke<LoopState[]>("loop_list");
}

export function loopGet(id: string): Promise<LoopState> {
  return invoke<LoopState>("loop_get", { id });
}

export function loopSetFeatures(id: string, titles: string[]): Promise<LoopState> {
  return invoke<LoopState>("loop_set_features", { id, titles });
}

export function loopSetContract(id: string, criteria: string[]): Promise<LoopState> {
  return invoke<LoopState>("loop_set_contract", { id, criteria });
}

export function loopReadyToEvaluate(id: string): Promise<LoopState> {
  return invoke<LoopState>("loop_ready_to_evaluate", { id });
}

export function loopGrade(id: string, verdicts: boolean[]): Promise<LoopState> {
  return invoke<LoopState>("loop_grade", { id, verdicts });
}

export function loopCurrentPrompt(id: string, diff: string): Promise<RolePrompt | null> {
  return invoke<RolePrompt | null>("loop_current_prompt", { id, diff });
}

/** The Summarizer role + prompt for compacting the loop's progress memory (not a phase). */
export function loopCompactPrompt(id: string): Promise<RolePrompt> {
  return invoke<RolePrompt>("loop_compact_prompt", { id });
}

/** Replace the loop's progress memory with the summarizer agent's compacted summary. */
export function loopCompact(id: string, agentId: string): Promise<LoopState> {
  return invoke<LoopState>("loop_compact", { id, agentId });
}

/** Parse the decomposer agent's output into the feature backlog and advance to planning. */
export function loopApplyDecomposer(id: string, agentId: string): Promise<LoopState> {
  return invoke<LoopState>("loop_apply_decomposer", { id, agentId });
}

/** Parse the planner agent's output into a contract and advance to generating. */
export function loopApplyPlanner(id: string, agentId: string): Promise<LoopState> {
  return invoke<LoopState>("loop_apply_planner", { id, agentId });
}

/** Parse the evaluator agent's PASS/FAIL verdicts and grade (pass / retry / fail). */
export function loopApplyEvaluator(id: string, agentId: string): Promise<LoopState> {
  return invoke<LoopState>("loop_apply_evaluator", { id, agentId });
}

// --- Task splitter (Phase 10): pre-launch parallel-decomposition classifier ---

/** One independently-runnable slice of a task, launched as its own agent. */
export interface TaskUnit {
  title: string;
  prompt: string;
  mentions: string[];
}

/** The classifier's verdict on how to run a task. */
export interface TaskPlan {
  /** Inferred 1–10 difficulty (feeds the difficulty heuristic). */
  difficulty: number;
  /** Whether the units are independent enough to run in parallel (false when only one). */
  parallel: boolean;
  units: TaskUnit[];
  rationale: string;
}

/** The classifier prompt for `task` (built in Rust so the wording lives in one place). */
export function taskSplitPrompt(task: string, projects: string[]): Promise<string> {
  return invoke<string>("task_split_prompt", { task, projects });
}

/** Parse a finished classifier agent's output into a plan, or null if it had no plan block. */
export function taskSplitParse(agentId: string): Promise<TaskPlan | null> {
  return invoke<TaskPlan | null>("task_split_parse", { agentId });
}
