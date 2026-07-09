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

/** The two barb endpoints of an arrowhead for a line from `from` to `to`. */
export function arrowHead(from: Point, to: Point, len = 14): [Point, Point] {
  const angle = Math.atan2(to.y - from.y, to.x - from.x);
  return [
    { x: to.x - len * Math.cos(angle - Math.PI / 6), y: to.y - len * Math.sin(angle - Math.PI / 6) },
    { x: to.x - len * Math.cos(angle + Math.PI / 6), y: to.y - len * Math.sin(angle + Math.PI / 6) },
  ];
}
