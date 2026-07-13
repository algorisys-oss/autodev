import { For, Show, type Component } from "solid-js";
import type { AgentEvent } from "../lib/ipc";
import type { AgentView } from "../lib/agent-store";

/** Renders a Rich (structured) agent session as a stream of cards, driven by the normalized
 *  `AgentEvent`s the store collects from `agent://event`. The read-only counterpart to
 *  TerminalPane: no keystrokes go back — a Rich session is one-shot (increment 1). */
export const RichPane: Component<{
  agentId: string;
  store: { state: { agents: AgentView[] } };
}> = (props) => {
  const agent = () => props.store.state.agents.find((a) => a.id === props.agentId);
  const events = () => agent()?.events ?? [];

  return (
    <div class="rich-pane">
      <Show
        when={events().length}
        fallback={<p class="rich-empty muted">Waiting for the agent’s first event…</p>}
      >
        <For each={events()}>{(ev) => <EventCard ev={ev} />}</For>
      </Show>
    </div>
  );
};

const EventCard: Component<{ ev: AgentEvent }> = (props) => {
  const ev = props.ev;
  switch (ev.kind) {
    case "sessionInit":
      return (
        <div class="rich-card rich-init">
          <span class="rich-chip">{ev.model || "session"}</span>
          <span class="rich-chip">{ev.permissionMode}</span>
          <span class="rich-path muted">{ev.cwd}</span>
        </div>
      );
    case "assistantText":
      return <div class="rich-card rich-assistant">{ev.text}</div>;
    case "thinking":
      return (
        <details class="rich-card rich-thinking">
          <summary class="muted">Thinking</summary>
          <div>{ev.text}</div>
        </details>
      );
    case "toolCall":
      return (
        <div class="rich-card rich-tool">
          <span class="rich-tool-name">{ev.name}</span>
          <span class="rich-tool-arg">{summarizeInput(ev.input)}</span>
        </div>
      );
    case "toolResult":
      return (
        <details class="rich-card rich-result" classList={{ "rich-error": !ev.ok }}>
          <summary class="muted">{ev.ok ? "result" : "error"}</summary>
          <pre>{ev.output}</pre>
        </details>
      );
    case "done":
      return (
        <div class="rich-card rich-done" classList={{ "rich-error": !ev.ok }}>
          <span class="rich-chip">{ev.ok ? "done" : "failed"}</span>
          <Show when={ev.durationMs != null}>
            <span class="muted">{Math.round((ev.durationMs as number) / 100) / 10}s</span>
          </Show>
          <Show when={ev.costUsd != null}>
            <span class="muted">${(ev.costUsd as number).toFixed(4)}</span>
          </Show>
          <Show when={ev.text}>
            <span class="rich-done-text">{ev.text}</span>
          </Show>
        </div>
      );
    case "raw":
      return <div class="rich-card rich-raw muted">{ev.text}</div>;
  }
};

/** Best-effort one-line summary of a tool call's arguments — the common path/command keys,
 *  else compact JSON. Purely cosmetic; the full input is in the raw terminal view. */
function summarizeInput(input: unknown): string {
  if (input && typeof input === "object") {
    const o = input as Record<string, unknown>;
    for (const k of ["file_path", "path", "command", "pattern", "query", "url"]) {
      if (typeof o[k] === "string") return o[k] as string;
    }
    return JSON.stringify(o);
  }
  return input == null ? "" : String(input);
}
