import { describe, it, expect } from "vitest";
import { extFromMime } from "./recorder";

describe("extFromMime", () => {
  it("extracts the subtype", () => {
    expect(extFromMime("audio/webm")).toBe("webm");
    expect(extFromMime("audio/ogg; codecs=opus")).toBe("ogg");
    expect(extFromMime("audio/wav")).toBe("wav");
  });
  it("falls back to webm", () => {
    expect(extFromMime("")).toBe("webm");
    expect(extFromMime("video/mp4")).toBe("webm");
  });
});
