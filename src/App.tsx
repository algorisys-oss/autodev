import { createSignal, onMount, Show } from "solid-js";
import { appInfo, getSettings, setSettings, type AppInfo, type AppSettings } from "./lib/ipc";
import "./App.css";

function App() {
  const [info, setInfo] = createSignal<AppInfo | null>(null);
  const [settings, setLocalSettings] = createSignal<AppSettings | null>(null);
  const [error, setError] = createSignal<string | null>(null);

  onMount(async () => {
    try {
      setInfo(await appInfo());
      setLocalSettings(await getSettings());
    } catch (e) {
      setError(String(e));
    }
  });

  async function cycleTheme() {
    const current = settings();
    if (!current) return;
    const order: AppSettings["theme"][] = ["system", "light", "dark"];
    const next = order[(order.indexOf(current.theme) + 1) % order.length];
    try {
      setLocalSettings(await setSettings({ ...current, theme: next }));
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <main class="app-shell">
      <header class="app-header">
        <h1>AutoDev</h1>
        <Show when={info()} fallback={<span class="muted">connecting…</span>}>
          {(i) => <span class="muted">v{i().version}</span>}
        </Show>
      </header>

      <Show when={error()}>
        {(e) => <p class="error">Core error: {e()}</p>}
      </Show>

      <section class="panel">
        <p>Foundation is live. The Rust core and the UI are talking.</p>
        <Show when={settings()}>
          {(s) => (
            <button onClick={cycleTheme}>Theme: {s().theme} (click to cycle)</button>
          )}
        </Show>
      </section>
    </main>
  );
}

export default App;
