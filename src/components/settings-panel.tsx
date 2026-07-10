import { createSignal, onMount, Show } from "solid-js";
import { getSettings, setSettings, type AppSettings } from "../lib/ipc";

const DEFAULTS: AppSettings = {
  theme: "system",
  defaultEffort: "high",
  transcribeCommand: "",
  screenshotCommand: "",
  browserCommand: "",
};

/** Modal to view and edit app settings — including the pluggable shell commands (voice,
 *  screenshot, browser) that were previously only editable by hand in ~/.autodev/settings.json. */
export function SettingsPanel(props: { onClose: () => void }) {
  const [form, setForm] = createSignal<AppSettings>(DEFAULTS);
  const [error, setError] = createSignal<string | null>(null);
  const [status, setStatus] = createSignal<string | null>(null);
  const [loaded, setLoaded] = createSignal(false);

  onMount(async () => {
    try {
      const s = await getSettings();
      // Normalise nulls to empty strings so the inputs are controlled.
      setForm({
        ...s,
        transcribeCommand: s.transcribeCommand ?? "",
        screenshotCommand: s.screenshotCommand ?? "",
        browserCommand: s.browserCommand ?? "",
      });
    } catch (e) {
      setError(String(e));
    } finally {
      setLoaded(true);
    }
  });

  const patch = (p: Partial<AppSettings>) => setForm((f) => ({ ...f, ...p }));

  async function save() {
    setError(null);
    setStatus("saving…");
    const f = form();
    // Persist empty command fields as null — "not configured" rather than an empty command.
    const payload: AppSettings = {
      ...f,
      transcribeCommand: f.transcribeCommand?.trim() ? f.transcribeCommand.trim() : null,
      screenshotCommand: f.screenshotCommand?.trim() ? f.screenshotCommand.trim() : null,
      browserCommand: f.browserCommand?.trim() ? f.browserCommand.trim() : null,
    };
    try {
      await setSettings(payload);
      setStatus("saved");
    } catch (e) {
      setStatus(null);
      setError(String(e));
    }
  }

  return (
    <div class="annotator-backdrop" onClick={(e) => e.target === e.currentTarget && props.onClose()}>
      <div class="handoff-modal">
        <div class="handoff-head">
          <strong>Settings</strong>
          <span class="spacer" />
          <button onClick={props.onClose}>Close</button>
        </div>

        <Show when={loaded()} fallback={<p class="muted">loading…</p>}>
          <div class="settings-row">
            <label class="handoff-field">
              Theme
              <select
                value={form().theme}
                onChange={(e) => patch({ theme: e.currentTarget.value as AppSettings["theme"] })}
              >
                <option value="system">system</option>
                <option value="light">light</option>
                <option value="dark">dark</option>
              </select>
            </label>
            <label class="handoff-field">
              Default effort
              <select
                value={form().defaultEffort}
                onChange={(e) =>
                  patch({ defaultEffort: e.currentTarget.value as AppSettings["defaultEffort"] })
                }
              >
                <option value="high">high</option>
                <option value="extra-high">extra-high</option>
              </select>
            </label>
          </div>

          <p class="muted settings-note">
            Pluggable commands. Use <code>{"{file}"}</code> where the tool should substitute the
            working file path. Leave blank to disable that tool.
          </p>

          <label class="handoff-field">
            Transcribe (voice → text)
            <input
              type="text"
              value={form().transcribeCommand ?? ""}
              onInput={(e) => patch({ transcribeCommand: e.currentTarget.value })}
              placeholder="whisper-cli -f {file} -otxt -of {file} && cat {file}.txt"
            />
          </label>
          <label class="handoff-field">
            Screenshot
            <input
              type="text"
              value={form().screenshotCommand ?? ""}
              onInput={(e) => patch({ screenshotCommand: e.currentTarget.value })}
              placeholder="grim {file}  ·  scrot {file}  ·  screencapture {file}"
            />
          </label>
          <label class="handoff-field">
            Browser handoff
            <input
              type="text"
              value={form().browserCommand ?? ""}
              onInput={(e) => patch({ browserCommand: e.currentTarget.value })}
              placeholder="playwright-runner.js {file}"
            />
          </label>

          <div class="handoff-row">
            <button class="primary" onClick={save}>
              Save
            </button>
            <Show when={status()}>{(s) => <span class="muted">{s()}</span>}</Show>
            <Show when={error()}>{(e) => <span class="error">{e()}</span>}</Show>
          </div>
        </Show>
      </div>
    </div>
  );
}
