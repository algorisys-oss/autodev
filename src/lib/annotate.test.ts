import { describe, it, expect } from "vitest";
import { arrowHead, annotationBlock } from "./annotate";

describe("annotationBlock", () => {
  it("renders notes per screenshot as a markdown block", () => {
    const block = annotationBlock([
      { image: "/a.png", notes: ["button misaligned", "  ", "contrast too low"] },
      { image: "/b.png", notes: ["missing label"] },
    ]);
    expect(block).toBe(
      "\n\n## Annotations\n" +
        "### Screenshot 1 (/a.png)\n1. button misaligned\n2. contrast too low\n" +
        "### Screenshot 2 (/b.png)\n1. missing label",
    );
  });

  it("is empty when there are no notes (blank notes and imageless captures don't count)", () => {
    expect(annotationBlock([])).toBe("");
    expect(annotationBlock([{ image: "/a.png", notes: ["", "   "] }])).toBe("");
  });
});

describe("arrowHead", () => {
  it("points backward from the tip and is symmetric for a horizontal line", () => {
    const [a, b] = arrowHead({ x: 0, y: 0 }, { x: 100, y: 0 }, 14);
    // both barbs sit behind the tip
    expect(a.x).toBeLessThan(100);
    expect(b.x).toBeLessThan(100);
    // symmetric about the x-axis
    expect(a.y).toBeCloseTo(-b.y, 5);
    expect(a.x).toBeCloseTo(b.x, 5);
  });
});
