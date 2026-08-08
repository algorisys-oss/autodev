import { createSignal } from "solid-js";
import type { HookBus } from "./hooks";

/** Audible completion notice: ring a short chime when an agent finishes a turn.
 *
 *  Wired through the P3 hook bus rather than the agent store, so it is one more consumer of
 *  the same seam as skills and extensions. Enabled state lives in a module-level signal (like
 *  `theme.ts`) so the Settings checkbox takes effect immediately instead of on restart; it is
 *  seeded from `AppSettings.bellOnComplete` at startup. */

const [bellEnabled, setBellEnabled] = createSignal(false);

export { bellEnabled, setBellEnabled };

/** Play a two-note chime, synthesized rather than loaded — no audio asset to ship, no decode
 *  step, and it works offline on every platform the webview runs on. */
export function ringBell(): void {
  const Ctor =
    globalThis.AudioContext ??
    (globalThis as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
  if (!Ctor) return;
  const ctx = new Ctor();
  const start = ctx.currentTime;
  for (const [freq, offset] of [
    [880, 0],
    [1318.5, 0.11],
  ]) {
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.type = "sine";
    osc.frequency.value = freq;
    const at = start + offset;
    // Struck-then-decaying envelope: a bell, not a beep. Exponential ramps cannot reach 0.
    gain.gain.setValueAtTime(0.0001, at);
    gain.gain.exponentialRampToValueAtTime(0.22, at + 0.01);
    gain.gain.exponentialRampToValueAtTime(0.0001, at + 0.7);
    osc.connect(gain);
    gain.connect(ctx.destination);
    osc.start(at);
    osc.stop(at + 0.75);
  }
  setTimeout(() => void ctx.close().catch(() => {}), 1200);
}

/** Ring on every completion an agent reports: it settled (finished its turn) or its process
 *  ended. An exit that immediately follows an idle is the same completion, so it is not rung
 *  twice; an agent killed while waiting, or a one-shot that exits without settling, still rings. */
export function installBellHook(
  bus: HookBus,
  deps: { enabled?: () => boolean; ring?: () => void } = {},
): void {
  const enabled = deps.enabled ?? bellEnabled;
  const ring = deps.ring ?? ringBell;
  const lastState = new Map<string, "idle" | "waiting">();

  bus.onIdle((id) => {
    lastState.set(id, "idle");
    if (enabled()) ring();
  });
  bus.onWaiting((id) => {
    lastState.set(id, "waiting");
  });
  bus.onExit((id) => {
    const announced = lastState.get(id) === "idle";
    lastState.delete(id);
    if (enabled() && !announced) ring();
  });
}
