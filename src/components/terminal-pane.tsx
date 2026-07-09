import { onMount, onCleanup } from "solid-js";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { agentWrite, agentResize } from "../lib/ipc";
import type { createAgentStore } from "../lib/agent-store";

/** Renders one agent's PTY. Output comes from the store (buffer replay + live stream);
 *  keystrokes go straight to the core; the PTY size tracks the pane. */
export function TerminalPane(props: {
  agentId: string;
  store: ReturnType<typeof createAgentStore>;
}) {
  let container!: HTMLDivElement;

  onMount(() => {
    const term = new Terminal({
      fontSize: 13,
      fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Consolas, "Liberation Mono", monospace',
      cursorBlink: true,
      theme: { background: "#1a1a1a" },
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(container);
    fit.fit();

    term.onData((d) => void agentWrite(props.agentId, d).catch(() => {}));

    const syncSize = () => {
      try {
        fit.fit();
        void agentResize(props.agentId, term.cols, term.rows).catch(() => {});
      } catch {
        /* pane not measurable yet */
      }
    };
    const ro = new ResizeObserver(syncSize);
    ro.observe(container);
    syncSize();

    // Replay any buffered output, then stream live chunks.
    props.store.attach(props.agentId, (bytes) => term.write(bytes));

    onCleanup(() => {
      props.store.detach(props.agentId);
      ro.disconnect();
      term.dispose();
    });
  });

  return <div class="terminal-pane" ref={container} />;
}
