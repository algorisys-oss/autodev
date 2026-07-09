import { createSignal, onMount, Show, For } from "solid-js";
import {
  loopCreate,
  loopList,
  loopSetContract,
  loopReadyToEvaluate,
  loopGrade,
  loopCurrentPrompt,
  type LoopState,
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

  const active = () => loops().find((l) => l.id === activeId()) ?? null;

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
    } catch (e) {
      setError(String(e));
    }
  }

  /** Spawn the current phase's role as an agent in the loop's project dir. */
  async function runRole() {
    const l = active();
    if (!l) return;
    setError(null);
    try {
      const rp = await loopCurrentPrompt(l.id, "");
      if (!rp) return;
      await props.agents.spawn(
        {
          backend: "claude",
          cwd: l.projectDir,
          planMode: rp.role === "planner",
          initialPrompt: rp.prompt,
        },
        `loop:${rp.role}`,
      );
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
        <button class="primary" onClick={create}>
          New loop
        </button>
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
              <Show when={l.phase !== "passed" && l.phase !== "failed"}>
                <button onClick={runRole}>▶ Run {l.phase === "planning" ? "planner" : l.phase === "generating" ? "generator" : "evaluator"}</button>
              </Show>
            </div>

            <Show when={l.phase === "planning"}>
              <div class="loop-step">
                <p class="muted">
                  Run the planner, then paste its contract criteria here (one per line):
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
