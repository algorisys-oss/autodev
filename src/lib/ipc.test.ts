import { describe, it, expect, vi, beforeEach } from "vitest";

// Mock the Tauri core module before importing the wrappers under test.
const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import { appInfo, getSettings, setSettings, type AppSettings } from "./ipc";

describe("ipc wrappers", () => {
  beforeEach(() => invokeMock.mockReset());

  it("appInfo calls the app_info command", async () => {
    invokeMock.mockResolvedValue({ name: "AutoDev", version: "0.1.0" });
    const info = await appInfo();
    expect(invokeMock).toHaveBeenCalledWith("app_info");
    expect(info.name).toBe("AutoDev");
  });

  it("getSettings calls the get_settings command", async () => {
    invokeMock.mockResolvedValue({ theme: "system", defaultEffort: "high" });
    const s = await getSettings();
    expect(invokeMock).toHaveBeenCalledWith("get_settings");
    expect(s.defaultEffort).toBe("high");
  });

  it("setSettings passes the settings payload under the settings key", async () => {
    const next: AppSettings = { theme: "dark", defaultEffort: "extra-high" };
    invokeMock.mockResolvedValue(next);
    const saved = await setSettings(next);
    expect(invokeMock).toHaveBeenCalledWith("set_settings", { settings: next });
    expect(saved).toEqual(next);
  });
});
