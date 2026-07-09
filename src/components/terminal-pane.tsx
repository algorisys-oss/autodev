import { onMount, onCleanup } from "solid-js";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import "@xterm/xterm/css/xterm.css";
import { agentWrite, agentResize } from "../lib/ipc";
import { base64ToBytes } from "../lib/bytes";

interface OutputPayload {
  id: string;
  data: string;
}
interface ExitPayload {
  id: string;
  code: number | null;
}

/** Renders one agent's PTY: streams core output into xterm, sends keystrokes back,
 *  and keeps the PTY size in sync with the pane. */
export function TerminalPane(props: { agentId: string; onExit?: (code: number | null) => void }) {
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

    const unlisten: Promise<UnlistenFn>[] = [
      listen<OutputPayload>("agent://output", (e) => {
        if (e.payload.id === props.agentId) term.write(base64ToBytes(e.payload.data));
      }),
      listen<ExitPayload>("agent://exit", (e) => {
        if (e.payload.id === props.agentId) {
          term.write("\r\n\x1b[90m[process exited]\x1b[0m\r\n");
          props.onExit?.(e.payload.code);
        }
      }),
    ];

    onCleanup(() => {
      ro.disconnect();
      unlisten.forEach((p) => void p.then((u) => u()));
      term.dispose();
    });
  });

  return <div class="terminal-pane" ref={container} />;
}
