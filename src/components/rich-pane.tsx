import { For, Show, createSignal, type Component } from "solid-js";
import type { AgentEvent } from "../lib/ipc";
import type { AgentView } from "../lib/agent-store";
import { isTerminal } from "../lib/agent-store";

/** Renders a Rich (structured) agent session as a stream of cards, driven by the normalized
 *  `AgentEvent`s the store collects from `agent://event`. Each turn is one-shot; a follow-up
 *  composer at the bottom continues the same conversation via `--resume` (store.followUp), and
 *  the new turn's cards append to this same stream. */
export const RichPane: Component<{
  agentId: string;
  store: {
    state: { agents: AgentView[] };
    followUp: (agentId: string, text: string) => Promise<void>;
    respondApproval: (agentId: string, requestId: string, allow: boolean) => Promise<void>;
  };
}> = (props) => {
  const agent = () => props.store.state.agents.find((a) => a.id === props.agentId);
  const events = () => agent()?.events ?? [];
  // A follow-up can be sent once the conversation has a session id and the current turn is done.
  const canFollowUp = () => {
    const a = agent();
    return !!a?.sessionId && isTerminal(a.status);
  };

  const [draft, setDraft] = createSignal("");
  async function send() {
    const text = draft().trim();
    if (!text || !canFollowUp()) return;
    setDraft("");
    await props.store.followUp(props.agentId, text);
  }

  return (
    <div class="rich-session">
      <div class="rich-pane">
        <Show
          when={events().length}
          fallback={<p class="rich-empty muted">Waiting for the agent’s first event…</p>}
        >
          <For each={events()}>
            {(ev) => (
              <EventCard
                ev={ev}
                onRespond={(requestId, allow) =>
                  void props.store.respondApproval(props.agentId, requestId, allow)
                }
              />
            )}
          </For>
        </Show>
      </div>
      <div class="rich-followup">
        <textarea
          class="rich-followup-text"
          rows={1}
          placeholder={
            agent()?.sessionId
              ? isTerminal(agent()!.status)
                ? "Reply to continue this conversation… (Enter to send)"
                : "Working… follow-up enabled when the turn finishes"
              : "Follow-up available once the session starts"
          }
          value={draft()}
          onInput={(e) => setDraft(e.currentTarget.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              void send();
            }
          }}
          disabled={!canFollowUp()}
        />
        <button class="primary" onClick={() => void send()} disabled={!canFollowUp() || !draft().trim()}>
          Send
        </button>
      </div>
    </div>
  );
};

const EventCard: Component<{
  ev: AgentEvent;
  onRespond: (requestId: string, allow: boolean) => void;
}> = (props) => {
  const ev = props.ev;
  switch (ev.kind) {
    case "permissionRequest":
      return (
        <div class="rich-card rich-approval" classList={{ "rich-approval-done": !!ev.decision }}>
          <div class="rich-approval-head">
            <span class="rich-approval-icon" aria-hidden="true">
              🔐
            </span>
            <span class="rich-tool-name">{ev.toolName}</span>
            <span class="rich-tool-arg">{summarizeInput(ev.toolInput)}</span>
          </div>
          <Show
            when={!ev.decision}
            fallback={
              <span
                class="rich-approval-outcome"
                classList={{ "rich-error": ev.decision === "deny" }}
              >
                {ev.decision === "allow" ? "✓ approved" : "✕ denied"}
              </span>
            }
          >
            <div class="rich-approval-actions">
              <button class="primary" onClick={() => props.onRespond(ev.requestId, true)}>
                Approve
              </button>
              <button onClick={() => props.onRespond(ev.requestId, false)}>Deny</button>
            </div>
          </Show>
        </div>
      );
    case "sessionInit":
      return (
        <div class="rich-card rich-init">
          <Show when={ev.model}>
            <span class="rich-chip">{ev.model}</span>
          </Show>
          <Show when={ev.permissionMode}>
            <span class="rich-chip">{ev.permissionMode}</span>
          </Show>
          <Show when={ev.cwd}>
            <span class="rich-path muted">{ev.cwd}</span>
          </Show>
        </div>
      );
    case "userMessage":
      return <div class="rich-card rich-user">{ev.text}</div>;
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
