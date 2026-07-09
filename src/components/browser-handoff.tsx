import { createSignal, Show } from "solid-js";
import { generateHandoff, runBrowserHandoff } from "../lib/ipc";

/** Modal to compose a browser task, generate a structured handoff prompt, copy it into a
 *  browser AI, or run the configured browserCommand on it. */
export function BrowserHandoff(props: { onClose: () => void }) {
  const [task, setTask] = createSignal("");
  const [url, setUrl] = createSignal("");
  const [context, setContext] = createSignal("");
  const [handoff, setHandoff] = createSignal("");
  const [output, setOutput] = createSignal<string | null>(null);
  const [error, setError] = createSignal<string | null>(null);
  const [busy, setBusy] = createSignal(false);

  async function generate() {
    setError(null);
    setOutput(null);
    setHandoff(await generateHandoff(task(), url(), context()));
  }

  async function copy() {
    try {
      await navigator.clipboard.writeText(handoff());
      setOutput("copied to clipboard");
    } catch (e) {
      setError(String(e));
    }
  }

  async function run() {
    setError(null);
    setBusy(true);
    try {
      setOutput(await runBrowserHandoff(handoff()));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div class="annotator-backdrop" onClick={(e) => e.target === e.currentTarget && props.onClose()}>
      <div class="handoff-modal">
        <div class="handoff-head">
          <strong>Browser handoff</strong>
          <span class="spacer" />
          <button onClick={props.onClose}>Close</button>
        </div>

        <label class="handoff-field">
          Task
          <textarea
            rows={2}
            value={task()}
            onInput={(e) => setTask(e.currentTarget.value)}
            placeholder="e.g. Create a Discord application and enable OAuth"
          />
        </label>
        <div class="handoff-row">
          <label class="handoff-field grow">
            Starting URL
            <input value={url()} onInput={(e) => setUrl(e.currentTarget.value)} placeholder="https://…" />
          </label>
        </div>
        <label class="handoff-field">
          Context (optional)
          <textarea
            rows={2}
            value={context()}
            onInput={(e) => setContext(e.currentTarget.value)}
            placeholder="Anything the browser AI should know"
          />
        </label>

        <div class="handoff-actions">
          <button class="primary" onClick={generate}>
            Generate handoff
          </button>
          <Show when={handoff()}>
            <button onClick={copy}>Copy</button>
            <button onClick={run} disabled={busy()}>
              {busy() ? "Running…" : "Run browserCommand"}
            </button>
          </Show>
        </div>

        <Show when={handoff()}>
          <textarea class="handoff-output" rows={10} readonly value={handoff()} />
        </Show>
        <Show when={output()}>{(o) => <pre class="handoff-result">{o()}</pre>}</Show>
        <Show when={error()}>{(e) => <p class="error">{e()}</p>}</Show>
      </div>
    </div>
  );
}
