export type Tool = "pen" | "rect" | "arrow";

export interface Point {
  x: number;
  y: number;
}

export interface Shape {
  tool: Tool;
  color: string;
  points: Point[];
}

/** A captured annotation (P9): an annotated screenshot path plus structured text notes. Unlike
 *  a bare image, the notes reach *every* backend as prompt text — including ones that ignore
 *  image attachments — and the same artifact fans out to all agents in a launch. */
export interface Annotation {
  /** Saved PNG path of the annotated screenshot. */
  image: string;
  /** Free-text notes about the capture, one per entry. */
  notes: string[];
}

/** Render the annotations' notes as a markdown block to append to an agent's prompt, so
 *  structured visual feedback travels as text on any backend. Empty string when no notes. */
export function annotationBlock(annotations: Annotation[]): string {
  const withNotes = annotations
    .map((a) => ({ image: a.image, notes: a.notes.map((n) => n.trim()).filter(Boolean) }))
    .filter((a) => a.notes.length);
  if (!withNotes.length) return "";
  const lines = ["", "## Annotations"];
  withNotes.forEach((a, i) => {
    lines.push(`### Screenshot ${i + 1} (${a.image})`);
    a.notes.forEach((n, j) => lines.push(`${j + 1}. ${n}`));
  });
  return "\n" + lines.join("\n");
}

/** The two barb endpoints of an arrowhead for a line from `from` to `to`. */
export function arrowHead(from: Point, to: Point, len = 14): [Point, Point] {
  const angle = Math.atan2(to.y - from.y, to.x - from.x);
  return [
    { x: to.x - len * Math.cos(angle - Math.PI / 6), y: to.y - len * Math.sin(angle - Math.PI / 6) },
    { x: to.x - len * Math.cos(angle + Math.PI / 6), y: to.y - len * Math.sin(angle + Math.PI / 6) },
  ];
}
