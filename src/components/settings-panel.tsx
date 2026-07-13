import { createSignal, onMount, Show } from "solid-js";
import { getSettings, setSettings, type AppSettings } from "../lib/ipc";
import { getThemePref, setThemePref, type ThemePref } from "../lib/theme";

const DEFAULTS: AppSettings = {
  theme: "system",
  defaultEffort: "high",
  transcribeCommand: "",
  screenshotCommand: "",
  browserCommand: "",
  editorCommand: "",
  autoSplitOnLaunch: false,
};

/** Modal to view and edit app settings — including the pluggable shell commands (voice,
 *  screenshot, browser) that were previously only editable by hand in ~/.autodev/settings.json. */
export function SettingsPanel(props: { onClose: () => void }) {
  const [form, setForm] = createSignal<AppSettings>(DEFAULTS);
  // Theme is applied live (attribute-driven) via the theme module, so it stays in sync with the
  // header toggle rather than only taking effect on Save.
  const [theme, setTheme] = createSignal<ThemePref>(getThemePref());
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
        editorCommand: s.editorCommand ?? "",
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
      editorCommand: f.editorCommand?.trim() ? f.editorCommand.trim() : null,
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
                value={theme()}
                onChange={(e) => {
                  const t = e.currentTarget.value as ThemePref;
                  setTheme(t);
                  setThemePref(t); // apply immediately, in sync with the header toggle
                }}
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
          <label class="handoff-field">
            Editor (for "Open in editor" — path is appended, no <code>{"{file}"}</code>)
            <input
              type="text"
              value={form().editorCommand ?? ""}
              onInput={(e) => patch({ editorCommand: e.currentTarget.value })}
              placeholder="code  ·  code -n  ·  cursor  ·  subl   (blank = code)"
            />
          </label>

          <label class="settings-check">
            <input
              type="checkbox"
              checked={form().autoSplitOnLaunch ?? false}
              onChange={(e) => patch({ autoSplitOnLaunch: e.currentTarget.checked })}
            />
            Auto-split on Launch — analyze the task for a parallel split before fanning out
            (unless already split or the agent count was set by hand)
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
