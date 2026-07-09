import { describe, it, expect } from "vitest";
import { base64ToBytes } from "./bytes";

describe("base64ToBytes", () => {
  it("decodes ascii", () => {
    expect(new TextDecoder().decode(base64ToBytes("aGVsbG8="))).toBe("hello");
  });

  it("preserves terminal escape bytes exactly", () => {
    // ESC [ 3 1 m  (a red-color SGR sequence) round-trips byte-for-byte
    const b64 = btoa("\x1b[31m");
    expect(Array.from(base64ToBytes(b64))).toEqual([0x1b, 0x5b, 0x33, 0x31, 0x6d]);
  });

  it("handles empty input", () => {
    expect(base64ToBytes("").length).toBe(0);
  });
});
