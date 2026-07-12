import * as ipcModule from "./ipc";
import type { AgentBackend, TaskPlan } from "./ipc";
import type { Subscribe } from "./agent-store";

/** What to analyze. `projects` are workspace project names the classifier may cite in a
 *  unit's `mentions`; `addDirs` are resolved @mention paths given to the classifier as
 *  extra read context. */
export interface AnalyzeParams {
  task: string;
  cwd: string;
  backend: AgentBackend;
  projects: string[];
  addDirs?: string[];
}

type AnalyzeApi = Pick<
  typeof ipcModule,
  "taskSplitPrompt" | "agentSpawn" | "taskSplitParse" | "agentKill"
>;

export interface AnalyzeDeps {
  api?: AnalyzeApi;
  subscribe?: Subscribe;
  /** Give up waiting for the classifier after this long, then parse whatever it produced. */
  timeoutMs?: number;
}

async function tauriSubscribe<T>(event: string, cb: (payload: T) => void): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<T>(event, (e) => cb(e.payload));
}

/** Resolve once the agent `id` emits `agent://exit`, or after `timeoutMs`. Resolves `true`
 *  on a real exit, `false` on timeout — the caller parses the log either way, so a missed
 *  event or a slow-but-complete run still yields a plan. */
function waitForExit(subscribe: Subscribe, id: string, timeoutMs: number): Promise<boolean> {
  return new Promise((resolve) => {
    let done = false;
    let unlisten: (() => void) | undefined;
    const finish = (v: boolean) => {
      if (done) return;
      done = true;
      clearTimeout(timer);
      unlisten?.();
      resolve(v);
    };
    const timer = setTimeout(() => finish(false), timeoutMs);
    void subscribe<{ id: string; code: number | null }>("agent://exit", (p) => {
      if (p.id === id) finish(true);
    }).then((u) => {
      unlisten = u;
      if (done) u(); // exit already fired before the subscription resolved
    });
  });
}

/** Run the one-shot task-splitter classifier and return its plan, or `null` if it produced
 *  none. The classifier agent runs in print mode and is NOT added to the agent grid — it is
 *  a throwaway analysis. The Rust ProcessManager still owns it and kills it on quit; a hung
 *  run is killed here on timeout. Nothing is launched: the caller reviews the plan first. */
export async function analyzeTask(
  params: AnalyzeParams,
  deps: AnalyzeDeps = {},
): Promise<TaskPlan | null> {
  const api = deps.api ?? ipcModule;
  const subscribe = deps.subscribe ?? tauriSubscribe;
  const timeoutMs = deps.timeoutMs ?? 120_000;

  const prompt = await api.taskSplitPrompt(params.task, params.projects);
  const id = await api.agentSpawn({
    backend: params.backend,
    cwd: params.cwd,
    printMode: true,
    addDirs: params.addDirs,
    initialPrompt: prompt,
  });

  const exited = await waitForExit(subscribe, id, timeoutMs);
  if (!exited) await Promise.resolve(api.agentKill(id)).catch(() => {});
  try {
    return await api.taskSplitParse(id);
  } catch {
    return null;
  }
}
