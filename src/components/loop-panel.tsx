import { createSignal, createEffect, onMount, Show, For } from "solid-js";
import {
  loopCreate,
  loopList,
  loopSetContract,
  loopReadyToEvaluate,
  loopGrade,
  loopCurrentPrompt,
  loopApplyPlanner,
  loopApplyEvaluator,
  type LoopState,
  type Role,
} from "../lib/ipc";
import type { createAgentStore } from "../lib/agent-store";

/** Drives the Planner → Generator → Evaluator loop (LOOPS Tier 6). Each role runs as a
 *  real agent in the loop's project dir; the contract and state live on disk. */
export function LoopPanel(props: {
  agents: ReturnType<typeof createAgentStore>;
  defaultProjectDir: string | null;
}) {
  const [loops, setLoops] = createSignal<LoopState[]>([]);
  const [activeId, setActiveId] = createSignal<string | null>(null);
  const [spec, setSpec] = createSignal("");
  const [criteriaText, setCriteriaText] = createSignal("");
  const [error, setError] = createSignal<string | null>(null);
  // The role agent currently running for a loop; when it exits we parse its output and advance.
  const [roleAgent, setRoleAgent] = createSignal<{
    loopId: string;
    agentId: string;
    role: Role;
  } | null>(null);
  const [applying, setApplying] = createSignal(false);
  // Off by default: when on, the loop spawns the next role itself after each advance, so a run
  // is hands-off end to end. Auto-launching a chain of agents is never the silent default.
  const [autoRun, setAutoRun] = createSignal(false);

  const active = () => loops().find((l) => l.id === activeId()) ?? null;
  const roleRunning = (loopId: string) => roleAgent()?.loopId === loopId;
  const inProgress = (phase: LoopState["phase"]) =>
    phase === "planning" || phase === "generating" || phase === "evaluating";

  // Auto-advance: when the tracked role agent exits, parse what it printed and move the loop
  // on (planner → contract, generator → evaluating, evaluator → graded). On a parse failure
  // the phase is left where it was so the manual controls below act as the fallback.
  createEffect(() => {
    const ra = roleAgent();
    if (!ra) return;
    const agent = props.agents.state.agents.find((a) => a.id === ra.agentId);
    if (!agent || agent.status !== "exited") return;
    setRoleAgent(null);
    void autoAdvance(ra);
  });

  async function autoAdvance(ra: { loopId: string; agentId: string; role: Role }) {
    setApplying(true);
    setError(null);
    try {
      let next: LoopState;
      if (ra.role === "planner") {
        next = await loopApplyPlanner(ra.loopId, ra.agentId);
      } else if (ra.role === "generator") {
        next = await loopReadyToEvaluate(ra.loopId);
      } else {
        next = await loopApplyEvaluator(ra.loopId, ra.agentId);
      }
      replace(next);
      // Hands-off: chain straight into the next role while the loop is still in progress.
      if (autoRun() && inProgress(next.phase)) await runRoleFor(next);
    } catch (e) {
      setError(`Auto-advance failed — continue manually below. (${String(e)})`);
    } finally {
      setApplying(false);
    }
  }

  onMount(refresh);
  async function refresh() {
    try {
      const list = await loopList();
      setLoops(list);
      if (!activeId() && list.length) setActiveId(list[0].id);
    } catch (e) {
      setError(String(e));
    }
  }

  function replace(l: LoopState) {
    setLoops((ls) => {
      const i = ls.findIndex((x) => x.id === l.id);
      if (i < 0) return [...ls, l];
      const copy = [...ls];
      copy[i] = l;
      return copy;
    });
  }

  async function create() {
    setError(null);
    if (!spec().trim()) return;
    if (!props.defaultProjectDir) {
      setError("Select a workspace with a project first.");
      return;
    }
    try {
      const l = await loopCreate(spec().trim(), props.defaultProjectDir);
      replace(l);
      setActiveId(l.id);
      setSpec("");
      if (autoRun()) await runRoleFor(l);
    } catch (e) {
      setError(String(e));
    }
  }

  /** Spawn the active loop's current-phase role. */
  async function runRole() {
    const l = active();
    if (l) await runRoleFor(l);
  }

  /** Spawn the given loop's current-phase role as an agent in its project dir. */
  async function runRoleFor(l: LoopState) {
    if (roleRunning(l.id)) return;
    setError(null);
    try {
      const rp = await loopCurrentPrompt(l.id, "");
      if (!rp) return;
      const agentId = await props.agents.spawn(
        {
          backend: "claude",
          cwd: l.projectDir,
          planMode: rp.role === "planner",
          initialPrompt: rp.prompt,
        },
        `loop:${rp.role}`,
      );
      setRoleAgent({ loopId: l.id, agentId, role: rp.role });
    } catch (e) {
      setError(String(e));
    }
  }

  async function setContract() {
    const l = active();
    if (!l) return;
    const criteria = criteriaText()
      .split("\n")
      .map((s) => s.trim())
      .filter(Boolean);
    if (!criteria.length) return;
    try {
      replace(await loopSetContract(l.id, criteria, []));
      setCriteriaText("");
    } catch (e) {
      setError(String(e));
    }
  }

  async function readyToEvaluate() {
    const l = active();
    if (l) replace(await loopReadyToEvaluate(l.id));
  }

  async function grade() {
    const l = active();
    if (!l) return;
    const verdicts = l.contract.map((c) => c.met === true);
    replace(await loopGrade(l.id, verdicts));
  }

  function toggleMet(i: number) {
    const l = active();
    if (!l) return;
    const next = { ...l, contract: l.contract.map((c, j) => (j === i ? { ...c, met: !(c.met === true) } : c)) };
    replace(next);
  }

  return (
    <div class="loop-panel">
      <div class="loop-new">
        <textarea
          rows={2}
          value={spec()}
          onInput={(e) => setSpec(e.currentTarget.value)}
          placeholder="Spec for a new autonomous loop (e.g. build a URL shortener with tests)…"
        />
        <div class="loop-new-actions">
          <button class="primary" onClick={create}>
            New loop
          </button>
          <label class="auto-run" title="Spawn each next role automatically until the loop passes or fails">
            <input
              type="checkbox"
              checked={autoRun()}
              onChange={(e) => setAutoRun(e.currentTarget.checked)}
            />
            Auto-run
          </label>
        </div>
      </div>

      <Show when={loops().length}>
        <div class="loop-tabs">
          <For each={loops()}>
            {(l) => (
              <button
                classList={{ active: l.id === activeId() }}
                onClick={() => setActiveId(l.id)}
                title={l.spec}
              >
                {l.spec.slice(0, 24)} · <span class={`phase ${l.phase}`}>{l.phase}</span>
              </button>
            )}
          </For>
        </div>
      </Show>

      <Show when={error()}>{(e) => <p class="error">{e()}</p>}</Show>

      <Show when={active()} keyed>
        {(l) => (
          <div class="loop-detail">
            <div class="loop-meta">
              <span class={`phase ${l.phase}`}>{l.phase}</span>
              <span class="muted">iteration {l.iteration}/{l.maxIterations}</span>
              <span class="spacer" />
              <Show when={roleRunning(l.id)}>
                <span class="muted">running {roleAgent()?.role}…</span>
              </Show>
              <Show when={applying()}>
                <span class="muted">reading output…</span>
              </Show>
              <Show when={l.phase !== "passed" && l.phase !== "failed"}>
                <button onClick={runRole} disabled={roleRunning(l.id) || applying()}>
                  ▶ Run {l.phase === "planning" ? "planner" : l.phase === "generating" ? "generator" : "evaluator"}
                </button>
              </Show>
            </div>

            <Show when={l.phase === "planning"}>
              <div class="loop-step">
                <p class="muted">
                  Run the planner — its contract is applied automatically when it finishes. If
                  parsing fails, paste the criteria here instead (one per line):
                </p>
                <textarea
                  rows={5}
                  value={criteriaText()}
                  onInput={(e) => setCriteriaText(e.currentTarget.value)}
                  placeholder="reject a missing email with 400&#10;short code is 6 chars&#10;…"
                />
                <button onClick={setContract}>Set contract & start generating</button>
              </div>
            </Show>

            <Show when={l.contract.length}>
              <ul class="contract">
                <For each={l.contract}>
                  {(c, i) => (
                    <li>
                      <Show
                        when={l.phase === "evaluating"}
                        fallback={
                          <span class={`check ${c.met === true ? "ok" : c.met === false ? "bad" : ""}`}>
                            {c.met === true ? "✓" : c.met === false ? "✗" : "•"}
                          </span>
                        }
                      >
                        <input type="checkbox" checked={c.met === true} onChange={() => toggleMet(i())} />
                      </Show>
                      <span>{c.text}</span>
                    </li>
                  )}
                </For>
              </ul>
            </Show>

            <Show when={l.phase === "generating"}>
              <button onClick={readyToEvaluate}>Generation done → evaluate</button>
            </Show>
            <Show when={l.phase === "evaluating"}>
              <p class="muted">
                Run the evaluator — verdicts are graded automatically when it finishes. Adjust
                the checkboxes above and grade manually if the parse was off.
              </p>
              <button class="primary" onClick={grade}>
                Grade & advance
              </button>
            </Show>
            <Show when={l.phase === "passed"}>
              <p class="loop-pass">✓ Contract met.</p>
            </Show>
            <Show when={l.phase === "failed"}>
              <p class="error">✗ Out of iterations without meeting the contract.</p>
            </Show>
          </div>
        )}
      </Show>
    </div>
  );
}
