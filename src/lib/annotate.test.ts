import { describe, it, expect } from "vitest";
import { arrowHead } from "./annotate";

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
