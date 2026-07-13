import { describe, it, expect, beforeEach, vi } from "vitest";
import { getThemePref, resolveTheme, applyTheme, setThemePref } from "./theme";

function stubSystemDark(dark: boolean) {
  window.matchMedia = vi.fn().mockReturnValue({
    matches: dark,
    addEventListener: () => {},
    removeEventListener: () => {},
  }) as unknown as typeof window.matchMedia;
}

describe("theme", () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.removeAttribute("data-theme");
    stubSystemDark(false);
  });

  it("defaults to system and reads a saved preference", () => {
    expect(getThemePref()).toBe("system");
    localStorage.setItem("autodev-theme", "dark");
    expect(getThemePref()).toBe("dark");
    localStorage.setItem("autodev-theme", "bogus");
    expect(getThemePref()).toBe("system");
  });

  it("resolves explicit prefs directly and system via the OS", () => {
    expect(resolveTheme("light")).toBe("light");
    expect(resolveTheme("dark")).toBe("dark");
    stubSystemDark(true);
    expect(resolveTheme("system")).toBe("dark");
    stubSystemDark(false);
    expect(resolveTheme("system")).toBe("light");
  });

  it("applyTheme stamps data-theme on the root", () => {
    applyTheme("dark");
    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
    applyTheme("light");
    expect(document.documentElement.getAttribute("data-theme")).toBe("light");
  });

  it("setThemePref persists and applies, returning the effective theme", () => {
    expect(setThemePref("dark")).toBe("dark");
    expect(localStorage.getItem("autodev-theme")).toBe("dark");
    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
  });
});
