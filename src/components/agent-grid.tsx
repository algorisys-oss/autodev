import { For, Show } from "solid-js";
import { isTerminal, type createAgentStore, type AgentStatus } from "../lib/agent-store";

const STATUS_LABEL: Record<AgentStatus, string> = {
  running: "running",
  idle: "idle",
  waiting: "waiting",
  exited: "exited",
  error: "error",
};

/** Grid of agent cards. Click a card to focus its terminal; × closes an exited one. */
export function AgentGrid(props: { store: ReturnType<typeof createAgentStore> }) {
  const { store } = props;
  return (
    <Show when={store.state.agents.length}>
      <div class="agent-grid-header">
        <span class="muted">{store.state.agents.length} agent(s)</span>
        <button class="danger" onClick={() => store.killAll()}>
          Kill all
        </button>
      </div>
      <ul class="agent-grid">
        <For each={store.state.agents}>
          {(a) => (
            <li
              classList={{ card: true, focused: a.id === store.state.focusedId }}
              onClick={() => store.focus(a.id)}
            >
              <span class={`dot ${a.status}`} title={STATUS_LABEL[a.status]} />
              <div class="card-body">
                <div class="card-title">{a.label}</div>
                <div class="card-sub muted">
                  {a.backend} · {STATUS_LABEL[a.status]}
                  <Show when={isTerminal(a.status)}> ({a.exitCode ?? "?"})</Show>
                </div>
              </div>
              <Show when={isTerminal(a.status)}>
                <button
                  class="icon"
                  title="Close"
                  onClick={(e) => {
                    e.stopPropagation();
                    store.close(a.id);
                  }}
                >
                  ×
                </button>
              </Show>
            </li>
          )}
        </For>
      </ul>
    </Show>
  );
}
