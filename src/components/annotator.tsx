import { createSignal, onMount, For } from "solid-js";
import { arrowHead, type Point, type Shape, type Tool } from "../lib/annotate";

const COLORS = ["#ff3b30", "#ffcc00", "#34c759", "#0a84ff", "#ffffff"];

/** Modal to draw arrows/boxes/freehand on a captured screenshot, then attach it.
 *  `imageBase64` is a bare PNG (no data-uri prefix); `onAttach` receives the annotated
 *  PNG the same way. */
export function Annotator(props: {
  imageBase64: string;
  onAttach: (pngBase64: string) => void;
  onCancel: () => void;
}) {
  let canvas!: HTMLCanvasElement;
  const [tool, setTool] = createSignal<Tool>("arrow");
  const [color, setColor] = createSignal(COLORS[0]);
  const shapes: Shape[] = [];
  let img: HTMLImageElement;
  let drawing: Shape | null = null;
  let rafPending = false;

  onMount(() => {
    img = new Image();
    img.onload = () => {
      canvas.width = img.naturalWidth;
      canvas.height = img.naturalHeight;
      redraw();
    };
    img.src = `data:image/png;base64,${props.imageBase64}`;
  });

  function redraw() {
    rafPending = false;
    const ctx = canvas.getContext("2d");
    if (!ctx || !img) return;
    ctx.drawImage(img, 0, 0);
    for (const s of [...shapes, drawing].filter(Boolean) as Shape[]) drawShape(ctx, s);
  }

  function scheduleRedraw() {
    if (!rafPending) {
      rafPending = true;
      requestAnimationFrame(redraw);
    }
  }

  function drawShape(ctx: CanvasRenderingContext2D, s: Shape) {
    ctx.strokeStyle = s.color;
    ctx.lineWidth = 3;
    ctx.lineCap = "round";
    ctx.lineJoin = "round";
    const p = s.points;
    if (p.length === 0) return;
    if (s.tool === "pen") {
      ctx.beginPath();
      ctx.moveTo(p[0].x, p[0].y);
      for (const pt of p.slice(1)) ctx.lineTo(pt.x, pt.y);
      ctx.stroke();
    } else if (s.tool === "rect" && p.length >= 2) {
      const [a, b] = [p[0], p[p.length - 1]];
      ctx.strokeRect(a.x, a.y, b.x - a.x, b.y - a.y);
    } else if (s.tool === "arrow" && p.length >= 2) {
      const [a, b] = [p[0], p[p.length - 1]];
      ctx.beginPath();
      ctx.moveTo(a.x, a.y);
      ctx.lineTo(b.x, b.y);
      const [h1, h2] = arrowHead(a, b);
      ctx.moveTo(h1.x, h1.y);
      ctx.lineTo(b.x, b.y);
      ctx.lineTo(h2.x, h2.y);
      ctx.stroke();
    }
  }

  function toCanvas(e: PointerEvent): Point {
    const r = canvas.getBoundingClientRect();
    return {
      x: (e.clientX - r.left) * (canvas.width / r.width),
      y: (e.clientY - r.top) * (canvas.height / r.height),
    };
  }

  function onDown(e: PointerEvent) {
    canvas.setPointerCapture(e.pointerId);
    drawing = { tool: tool(), color: color(), points: [toCanvas(e)] };
    scheduleRedraw();
  }
  function onMove(e: PointerEvent) {
    if (!drawing) return;
    const pt = toCanvas(e);
    if (drawing.tool === "pen") drawing.points.push(pt);
    else drawing.points[1] = pt;
    scheduleRedraw();
  }
  function onUp() {
    if (drawing && drawing.points.length > 0) shapes.push(drawing);
    drawing = null;
    scheduleRedraw();
  }

  function undo() {
    shapes.pop();
    scheduleRedraw();
  }

  function attach() {
    const data = canvas.toDataURL("image/png").split(",")[1];
    props.onAttach(data);
  }

  return (
    <div class="annotator-backdrop" onClick={(e) => e.target === e.currentTarget && props.onCancel()}>
      <div class="annotator">
        <div class="annotator-toolbar">
          <For each={["arrow", "rect", "pen"] as Tool[]}>
            {(t) => (
              <button classList={{ active: tool() === t }} onClick={() => setTool(t)}>
                {t}
              </button>
            )}
          </For>
          <span class="tool-sep" />
          <For each={COLORS}>
            {(c) => (
              <button
                class="swatch"
                classList={{ active: color() === c }}
                style={{ background: c }}
                onClick={() => setColor(c)}
              />
            )}
          </For>
          <span class="spacer" />
          <button onClick={undo}>Undo</button>
          <button onClick={props.onCancel}>Cancel</button>
          <button class="primary" onClick={attach}>
            Attach
          </button>
        </div>
        <div class="annotator-canvas-wrap">
          <canvas
            ref={canvas}
            class="annotator-canvas"
            onPointerDown={onDown}
            onPointerMove={onMove}
            onPointerUp={onUp}
          />
        </div>
      </div>
    </div>
  );
}
