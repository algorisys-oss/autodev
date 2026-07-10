import { createSignal, createEffect, onMount, Show, For } from "solid-js";
import {
  loopCreate,
  loopList,
  loopSetFeatures,
  loopSetContract,
  loopReadyToEvaluate,
  loopGrade,
  loopCurrentPrompt,
  loopCompactPrompt,
  loopCompact,
  loopApplyDecomposer,
  loopApplyPlanner,
  loopApplyEvaluator,
  needsCompaction,
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
  const [verifyCommand, setVerifyCommand] = createSignal("");
  const [maxIterations, setMaxIterations] = createSignal(8);
  const [continueOnFailure, setContinueOnFailure] = createSignal(false);
  const [featuresText, setFeaturesText] = createSignal("");
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
  // Off by default: auto-accept Claude Code's "trust this folder?" prompt so an unattended run
  // doesn't stall on it. Sending keystrokes to an agent is never the silent default.
  const [autoOnboard, setAutoOnboard] = createSignal(false);

  const active = () => loops().find((l) => l.id === activeId()) ?? null;
  const roleRunning = (loopId: string) => roleAgent()?.loopId === loopId;
  const inProgress = (phase: LoopState["phase"]) =>
    phase === "decomposing" ||
    phase === "planning" ||
    phase === "generating" ||
    phase === "evaluating";

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
      // The summarizer is a maintenance step: it compacts memory, then the deferred phase role
      // runs. Every other role advances the phase machine.
      if (ra.role === "summarizer") {
        const next = await loopCompact(ra.loopId, ra.agentId);
        replace(next);
        if (autoRun() && inProgress(next.phase)) await runRoleFor(next);
        return;
      }
      let next: LoopState;
      if (ra.role === "decomposer") {
        next = await loopApplyDecomposer(ra.loopId, ra.agentId);
      } else if (ra.role === "planner") {
        next = await loopApplyPlanner(ra.loopId, ra.agentId);
      } else if (ra.role === "generator") {
        next = await loopReadyToEvaluate(ra.loopId);
      } else {
        next = await loopApplyEvaluator(ra.loopId, ra.agentId);
      }
      replace(next);
      // Hands-off: chain into the next role, compacting memory first if it has grown too large.
      if (autoRun() && inProgress(next.phase)) await runNextRole(next);
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
      const l = await loopCreate(
        spec().trim(),
        props.defaultProjectDir,
        verifyCommand().trim() || null,
        maxIterations(),
        continueOnFailure(),
      );
      replace(l);
      setActiveId(l.id);
      setSpec("");
      if (autoRun()) await runNextRole(l);
    } catch (e) {
      setError(String(e));
    }
  }

  /** Spawn the active loop's current-phase role. */
  async function runRole() {
    const l = active();
    if (l) await runRoleFor(l);
  }

  /** In the hands-off chain: compact the memory first if it has grown too large, otherwise run
   *  the current-phase role directly. */
  async function runNextRole(l: LoopState) {
    if (l.phase !== "decomposing" && needsCompaction(l.progress)) {
      await runSummarizerFor(l);
    } else {
      await runRoleFor(l);
    }
  }

  /** Spawn a read-only Summarizer to compact the loop's progress memory. */
  async function runSummarizerFor(l: LoopState) {
    if (roleRunning(l.id)) return;
    setError(null);
    try {
      const rp = await loopCompactPrompt(l.id);
      const agentId = await props.agents.spawn(
        { backend: "claude", cwd: l.projectDir, planMode: true, initialPrompt: rp.prompt },
        "loop:summarizer",
      );
      if (autoOnboard()) props.agents.setAutoOnboard(agentId, true);
      setRoleAgent({ loopId: l.id, agentId, role: "summarizer" });
    } catch (e) {
      setError(String(e));
    }
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
          // Decomposer and planner don't write code — run them read-only.
          planMode: rp.role === "decomposer" || rp.role === "planner",
          initialPrompt: rp.prompt,
        },
        `loop:${rp.role}`,
      );
      if (autoOnboard()) props.agents.setAutoOnboard(agentId, true);
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
      replace(await loopSetContract(l.id, criteria));
      setCriteriaText("");
    } catch (e) {
      setError(String(e));
    }
  }

  async function setFeatures() {
    const l = active();
    if (!l) return;
    const titles = featuresText()
      .split("\n")
      .map((s) => s.trim())
      .filter(Boolean);
    if (!titles.length) return;
    try {
      replace(await loopSetFeatures(l.id, titles));
      setFeaturesText("");
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
        <div class="loop-new-spec">
          <textarea
            rows={2}
            value={spec()}
            onInput={(e) => setSpec(e.currentTarget.value)}
            placeholder="Spec for a new autonomous loop (e.g. build a URL shortener with tests)…"
          />
          <div class="loop-new-config">
            <label class="loop-cfg" title="Ground truth: a command whose exit 0 = tests pass. Blocks a pass even if the evaluator rates every criterion PASS.">
              Verify command
              <input
                type="text"
                value={verifyCommand()}
                onInput={(e) => setVerifyCommand(e.currentTarget.value)}
                placeholder="./dev.sh verify  ·  npm test"
              />
            </label>
            <label class="loop-cfg" title="Max generate→evaluate rounds before the loop gives up (it also stops early if it stalls).">
              Max rounds
              <input
                type="number"
                min={1}
                value={maxIterations()}
                onInput={(e) => setMaxIterations(Math.max(1, Number(e.currentTarget.value) || 1))}
              />
            </label>
            <label
              class="loop-cfg loop-cfg-check"
              title="If a feature fails, skip it and keep building the rest of the backlog instead of failing the whole epic."
            >
              <input
                type="checkbox"
                checked={continueOnFailure()}
                onChange={(e) => setContinueOnFailure(e.currentTarget.checked)}
              />
              Continue on failure
            </label>
          </div>
        </div>
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
          <label class="auto-run" title="Auto-accept Claude Code's 'trust this folder?' prompt so an unattended run doesn't stall">
            <input
              type="checkbox"
              checked={autoOnboard()}
              onChange={(e) => setAutoOnboard(e.currentTarget.checked)}
            />
            Auto-onboard
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
              <Show when={l.features.length}>
                <span class="muted">
                  feature {Math.min((l.currentFeature ?? 0) + 1, l.features.length)}/
                  {l.features.length}
                </span>
              </Show>
              <span class="muted">round {l.iteration + 1}/{l.maxIterations}</span>
              <Show when={l.verifyCommand}>
                <span class="muted" title="Ground-truth test command — must pass to complete">
                  · verify: <code>{l.verifyCommand}</code>
                </span>
              </Show>
              <span class="spacer" />
              <Show when={roleRunning(l.id)}>
                <span class="muted">running {roleAgent()?.role}…</span>
              </Show>
              <Show when={applying()}>
                <span class="muted">reading output…</span>
              </Show>
              <Show when={l.phase !== "passed" && l.phase !== "failed" && needsCompaction(l.progress)}>
                <button
                  onClick={() => runSummarizerFor(l)}
                  disabled={roleRunning(l.id) || applying()}
                  title="Compress the accumulated progress memory with a summarizer agent"
                >
                  🗜 Compact memory
                </button>
              </Show>
              <Show when={l.phase !== "passed" && l.phase !== "failed"}>
                <button onClick={runRole} disabled={roleRunning(l.id) || applying()}>
                  ▶ Run{" "}
                  {l.phase === "decomposing"
                    ? "decomposer"
                    : l.phase === "planning"
                      ? "planner"
                      : l.phase === "generating"
                        ? "generator"
                        : "evaluator"}
                </button>
              </Show>
            </div>

            <Show when={l.features.length}>
              <ol class="feature-backlog">
                <For each={l.features}>
                  {(f, i) => (
                    <li
                      classList={{
                        done: f.done,
                        failed: !!f.failed,
                        current: !f.done && !f.failed && i() === (l.currentFeature ?? 0),
                      }}
                    >
                      <span class="feature-mark">
                        {f.done
                          ? "✓"
                          : f.failed
                            ? "✗"
                            : i() === (l.currentFeature ?? 0)
                              ? "▸"
                              : "•"}
                      </span>
                      <span>{f.title}</span>
                    </li>
                  )}
                </For>
              </ol>
            </Show>

            <Show when={l.phase === "decomposing"}>
              <div class="loop-step">
                <p class="muted">
                  Run the decomposer — it breaks the spec into an ordered feature backlog, applied
                  automatically when it finishes. If parsing fails, paste features here (one per
                  line):
                </p>
                <textarea
                  rows={5}
                  value={featuresText()}
                  onInput={(e) => setFeaturesText(e.currentTarget.value)}
                  placeholder="user auth&#10;create and list posts&#10;full-text search&#10;…"
                />
                <button onClick={setFeatures}>Set backlog & start planning</button>
              </div>
            </Show>

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
              <p class="loop-pass">
                ✓ {l.features.length ? `All ${l.features.length} features done` : "Contract met"}
                {l.verifyCommand ? " and tests pass." : "."}
              </p>
            </Show>
            <Show when={l.phase === "failed"}>
              <p class="error">✗ {l.failureReason ?? "Did not meet the contract."}</p>
            </Show>
          </div>
        )}
      </Show>
    </div>
  );
}
